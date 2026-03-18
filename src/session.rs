use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// Wraps the JSON value after `--parameters` in single quotes so the shell
/// does not interpret curly braces, square brackets, or double quotes.
fn quote_parameters_for_shell(command: &str) -> String {
    let re = Regex::new(r"--parameters\s+(\{.+\})").unwrap();
    if let Some(caps) = re.captures(command) {
        let json_val = &caps[1];
        command.replace(json_val, &format!("'{}'", json_val))
    } else {
        command.to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    Stopped,
    Starting,
    Running,
    Connected,
    Expired,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionKind {
    SsoLogin { session_name: String },
    SsmSession,
    Other,
}

#[derive(Debug)]
pub struct Session {
    pub status: SessionStatus,
    pub pid: Option<u32>,
    pub output_lines: Vec<String>,
    pub _kind: SessionKind,
    /// SSO: profile used for sts check, and last check result
    pub sso_profile: Option<String>,
    pub token_expires_at: Option<String>,
    pub token_remaining_secs: Option<u64>,
    child: Option<Child>,
}

impl Session {
    pub fn new(kind: SessionKind) -> Self {
        Self {
            status: SessionStatus::Stopped,
            pid: None,
            output_lines: Vec::new(),
            _kind: kind,
            sso_profile: None,
            token_expires_at: None,
            token_remaining_secs: None,
            child: None,
        }
    }
}

pub struct SessionManager {
    pub sessions: HashMap<String, Arc<Mutex<Session>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub async fn start_session(
        &mut self,
        alias_name: &str,
        command: &str,
        kind: SessionKind,
        output_tx: tokio::sync::mpsc::UnboundedSender<(String, String)>,
    ) -> Result<(), String> {
        if let Some(session) = self.sessions.get(alias_name) {
            let s = session.lock().await;
            match s.status {
                SessionStatus::Running | SessionStatus::Starting | SessionStatus::Connected => {
                    return Err("Session already active".to_string());
                }
                _ => {}
            }
        }

        let session = Arc::new(Mutex::new(Session::new(kind.clone())));

        {
            let mut s = session.lock().await;
            s.status = SessionStatus::Starting;
            s.output_lines.clear();
            s.token_expires_at = None;
            s.token_remaining_secs = None;
            s.sso_profile = None;
            s.output_lines.push(format!(">>> Starting: {}", command));

            let shell_command = quote_parameters_for_shell(command);
            let stdin_cfg = match kind {
                SessionKind::SsmSession => Stdio::piped(),
                _ => Stdio::null(),
            };
            let mut child = Command::new("sh")
                .arg("-c")
                .arg(&shell_command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(stdin_cfg)
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| format!("Failed to spawn: {}", e))?;

            s.pid = child.id();
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            s.child = Some(child);
            s.status = SessionStatus::Running;

            let name_clone = alias_name.to_string();
            let tx_clone = output_tx.clone();

            if let Some(stdout) = stdout {
                let name = name_clone.clone();
                let tx = tx_clone.clone();
                tokio::spawn(async move {
                    let reader = BufReader::new(stdout);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = tx.send((name.clone(), line));
                    }
                });
            }

            if let Some(stderr) = stderr {
                let name = name_clone.clone();
                let tx = tx_clone;
                tokio::spawn(async move {
                    let reader = BufReader::new(stderr);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = tx.send((name.clone(), format!("[stderr] {}", line)));
                    }
                });
            }
        }

        let session_clone = session.clone();
        let kind_clone = kind;
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let mut s = session_clone.lock().await;
                if let Some(ref mut child) = s.child {
                    match child.try_wait() {
                        Ok(Some(exit_status)) => {
                            s.child = None;
                            s.pid = None;

                            if exit_status.success() {
                                if let SessionKind::SsoLogin { ref session_name } = kind_clone {
                                    s.status = SessionStatus::Connected;
                                    s.output_lines.push(format!(
                                        ">>> SSO login succeeded (session: {})",
                                        session_name
                                    ));
                                    // Expiry will be seeded by the liveness watcher on first tick

                                    // Resolve profile from ~/.aws/config
                                    let profile = resolve_profile_for_sso_session(session_name);
                                    match &profile {
                                        Some(p) => {
                                            s.sso_profile = Some(p.clone());
                                            s.output_lines.push(format!(
                                                ">>> Monitoring via: aws sts get-caller-identity --profile {}",
                                                p
                                            ));
                                        }
                                        None => {
                                            s.output_lines.push(
                                                ">>> No profile found for this sso-session in ~/.aws/config".to_string(),
                                            );
                                            s.output_lines.push(
                                                ">>> Cannot monitor session liveness without a profile".to_string(),
                                            );
                                        }
                                    }
                                } else {
                                    s.status = SessionStatus::Stopped;
                                    s.output_lines
                                        .push(">>> Process exited successfully".to_string());
                                }
                            } else {
                                let msg = format!(
                                    "Process exited with code: {}",
                                    exit_status.code().unwrap_or(-1)
                                );
                                s.status = SessionStatus::Error(msg.clone());
                                s.output_lines.push(format!(">>> {}", msg));
                            }
                            break;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            s.status = SessionStatus::Error(format!("Monitor error: {}", e));
                            break;
                        }
                    }
                } else {
                    break;
                }
            }

            // For SSO sessions with a resolved profile, start liveness watcher
            if let SessionKind::SsoLogin { .. } = kind_clone {
                let profile = {
                    let s = session_clone.lock().await;
                    s.sso_profile.clone()
                };
                if let Some(profile) = profile {
                    let sess = session_clone.clone();
                    tokio::spawn(async move {
                        sso_liveness_watcher(sess, profile).await;
                    });
                }
            }
        });

        self.sessions.insert(alias_name.to_string(), session);
        Ok(())
    }

    pub async fn stop_session(&mut self, alias_name: &str) -> Result<(), String> {
        if let Some(session) = self.sessions.get(alias_name) {
            let mut s = session.lock().await;

            if matches!(s.status, SessionStatus::Connected) {
                s.status = SessionStatus::Stopped;
                s.token_expires_at = None;
                s.token_remaining_secs = None;
                s.output_lines.push(">>> SSO session dismissed".to_string());
                return Ok(());
            }

            if let Some(ref mut child) = s.child {
                child
                    .kill()
                    .await
                    .map_err(|e| format!("Failed to kill: {}", e))?;
                s.status = SessionStatus::Stopped;
                s.output_lines
                    .push(">>> Session stopped by user".to_string());
                s.child = None;
                s.pid = None;
                Ok(())
            } else {
                s.status = SessionStatus::Stopped;
                s.token_expires_at = None;
                s.token_remaining_secs = None;
                Ok(())
            }
        } else {
            Err("Session not found".to_string())
        }
    }

    pub async fn get_status(&self, alias_name: &str) -> SessionStatus {
        if let Some(session) = self.sessions.get(alias_name) {
            let s = session.lock().await;
            s.status.clone()
        } else {
            SessionStatus::Stopped
        }
    }

    pub async fn get_output(&self, alias_name: &str) -> Vec<String> {
        if let Some(session) = self.sessions.get(alias_name) {
            let s = session.lock().await;
            s.output_lines.clone()
        } else {
            Vec::new()
        }
    }

    pub async fn get_pid(&self, alias_name: &str) -> Option<u32> {
        if let Some(session) = self.sessions.get(alias_name) {
            let s = session.lock().await;
            s.pid
        } else {
            None
        }
    }

    pub async fn get_sso_profile(&self, alias_name: &str) -> Option<String> {
        if let Some(session) = self.sessions.get(alias_name) {
            let s = session.lock().await;
            s.sso_profile.clone()
        } else {
            None
        }
    }

    pub async fn get_token_expiry(&self, alias_name: &str) -> (Option<String>, Option<u64>) {
        if let Some(session) = self.sessions.get(alias_name) {
            let s = session.lock().await;
            (s.token_expires_at.clone(), s.token_remaining_secs)
        } else {
            (None, None)
        }
    }

    pub async fn append_output(&self, alias_name: &str, line: String) {
        if let Some(session) = self.sessions.get(alias_name) {
            let mut s = session.lock().await;
            s.output_lines.push(line);
            if s.output_lines.len() > 500 {
                let drain = s.output_lines.len() - 500;
                s.output_lines.drain(..drain);
            }
        }
    }

    pub async fn stop_all(&mut self) {
        let names: Vec<String> = self.sessions.keys().cloned().collect();
        for name in names {
            let _ = self.stop_session(&name).await;
        }
    }

    /// On startup: check which SSO sessions are already authenticated.
    /// For each SSO alias, resolve its profile and run sts get-caller-identity.
    /// If valid, create a session in Connected state and start the liveness watcher.
    pub async fn check_existing_sso_sessions(
        &mut self,
        aliases: &[(String, String)], // (alias_name, sso_session_name)
    ) {
        for (alias_name, sso_session_name) in aliases {
            let profile = match resolve_profile_for_sso_session(sso_session_name) {
                Some(p) => p,
                None => continue,
            };

            let result = check_sts_identity(&profile).await;

            match result {
                StsCheckResult::Valid { account, arn } => {
                    let kind = SessionKind::SsoLogin {
                        session_name: sso_session_name.clone(),
                    };
                    let session = Arc::new(Mutex::new(Session::new(kind)));

                    {
                        let mut s = session.lock().await;
                        s.status = SessionStatus::Connected;
                        s.sso_profile = Some(profile.clone());
                        s.token_expires_at = Some(format!("account: {}", account));
                        s.output_lines.push(format!(
                            ">>> Existing SSO session detected (profile: {})",
                            profile
                        ));
                        s.output_lines.push(format!(
                            ">>> Session verified — {} ({})",
                            arn, account
                        ));
                    }

                    // Seed expiry immediately on startup
                    if let Some((exp_str, remaining)) = read_credentials_expiry(&profile).await {
                        let mut s = session.lock().await;
                        s.token_expires_at = Some(exp_str);
                        s.token_remaining_secs = Some(remaining);
                    }

                    // Start liveness watcher
                    let sess_clone = session.clone();
                    let prof_clone = profile.clone();
                    tokio::spawn(async move {
                        sso_liveness_watcher(sess_clone, prof_clone).await;
                    });

                    self.sessions.insert(alias_name.clone(), session);
                }
                _ => {
                    // Not authenticated — leave as Stopped
                }
            }
        }
    }
}

// ─── Resolve profile name from ~/.aws/config ────────────────────────
// Parses ~/.aws/config to find a [profile X] that has sso_session = <name>

fn resolve_profile_for_sso_session(session_name: &str) -> Option<String> {
    let config_path = dirs::home_dir()?.join(".aws/config");
    let content = fs::read_to_string(config_path).ok()?;

    let mut current_profile: Option<String> = None;
    let target = session_name.to_lowercase();

    for line in content.lines() {
        let trimmed = line.trim();

        // Match [profile xyz] or [default]
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = trimmed[1..trimmed.len() - 1].trim();
            if inner.starts_with("profile ") {
                current_profile = Some(inner["profile ".len()..].trim().to_string());
            } else if inner == "default" {
                current_profile = Some("default".to_string());
            } else {
                // Could be [sso-session ...] section — skip
                current_profile = None;
            }
            continue;
        }

        // Match sso_session = <name>
        if let Some(ref profile) = current_profile {
            if let Some((key, value)) = trimmed.split_once('=') {
                let key = key.trim().to_lowercase();
                let value = value.trim().to_lowercase();
                if key == "sso_session" && value == target {
                    return Some(profile.clone());
                }
            }
        }
    }

    None
}

// ─── SSO Liveness Watcher ───────────────────────────────────────────
// Periodically runs `aws sts get-caller-identity --profile <profile>`
// to verify the SSO session is still valid, then reads the credential
// expiry via `aws configure export-credentials --profile <profile>`.

async fn sso_liveness_watcher(
    session: Arc<Mutex<Session>>,
    profile: String,
) {
    // Initial check after 5 seconds (give CLI time to cache credentials)
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    loop {
        let status = {
            let s = session.lock().await;
            s.status.clone()
        };

        if status != SessionStatus::Connected {
            break;
        }

        let check_result = check_sts_identity(&profile).await;
        // Read expiry via export-credentials AFTER STS (may trigger token refresh)
        let expiry = read_credentials_expiry(&profile).await;

        {
            let mut s = session.lock().await;

            if s.status != SessionStatus::Connected {
                break;
            }

            match check_result {
                StsCheckResult::Valid { account, arn } => {
                    match expiry {
                        Some((ref exp_str, remaining)) if remaining > 0 => {
                            s.token_expires_at = Some(exp_str.clone());
                            s.token_remaining_secs = Some(remaining);
                        }
                        _ => {
                            // export-credentials unavailable or returned expired;
                            // STS succeeded so session is live — show account instead.
                            s.token_expires_at = Some(format!("account: {}", account));
                        }
                    }
                    s.output_lines.push(format!(
                        ">>> Session verified — {} ({})",
                        arn, account
                    ));
                }
                StsCheckResult::Expired { error } => {
                    s.status = SessionStatus::Expired;
                    s.token_remaining_secs = Some(0);
                    s.output_lines.push(format!(">>> SSO session expired — {}", error));
                    s.output_lines.push(">>> Re-login required (press Enter)".to_string());
                    break;
                }
                StsCheckResult::Error { error } => {
                    s.output_lines.push(format!(
                        ">>> STS check failed: {} (will retry)",
                        error
                    ));
                }
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

// ─── Credential Expiry via AWS CLI ──────────────────────────────────
// Runs `aws configure export-credentials --profile <profile>` and
// extracts the Expiration field. This is profile-scoped and works
// correctly when multiple SSO sessions are active simultaneously.

async fn read_credentials_expiry(profile: &str) -> Option<(String, u64)> {
    let output = Command::new("aws")
        .args(["configure", "export-credentials", "--profile", profile])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).ok()?;
    let expiration = json.get("Expiration").and_then(|v| v.as_str())?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    let expiry_unix = parse_iso8601_to_unix(expiration)?;
    let remaining = expiry_unix.saturating_sub(now);
    Some((format_expiry(remaining), remaining))
}

/// Parse an ISO 8601 UTC timestamp to a Unix timestamp (seconds).
/// Handles: "2024-01-15T10:30:00UTC", "…Z", "…+00:00", "…+0000"
fn parse_iso8601_to_unix(s: &str) -> Option<u64> {
    let s = s.trim();
    // Strip timezone suffix
    let s = s
        .strip_suffix("UTC")
        .or_else(|| s.strip_suffix('Z'))
        .or_else(|| s.strip_suffix("+00:00"))
        .or_else(|| s.strip_suffix("+0000"))
        .unwrap_or(s);

    if s.len() < 19 {
        return None;
    }
    let year: i64  = s.get(0..4)?.parse().ok()?;
    let month: i64 = s.get(5..7)?.parse().ok()?;
    let day: i64   = s.get(8..10)?.parse().ok()?;
    let hour: i64  = s.get(11..13)?.parse().ok()?;
    let min: i64   = s.get(14..16)?.parse().ok()?;
    let sec: i64   = s.get(17..19)?.parse().ok()?;

    // Days since 1970-01-01 — Howard Hinnant's civil_from_days inverse
    let y   = if month <= 2 { year - 1 } else { year };
    let m   = month;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;

    if days < 0 {
        return None;
    }
    Some((days * 86400 + hour * 3600 + min * 60 + sec) as u64)
}

/// Format remaining seconds as a human-readable string: "3d 2h", "1h 14m", "45m"
pub fn format_expiry(remaining_secs: u64) -> String {
    if remaining_secs == 0 {
        return "expired".to_string();
    }
    let days  = remaining_secs / 86400;
    let hours = (remaining_secs % 86400) / 3600;
    let mins  = (remaining_secs % 3600) / 60;
    if days > 0        { format!("{}d {}h", days, hours) }
    else if hours > 0  { format!("{}h {}m", hours, mins) }
    else               { format!("{}m", mins.max(1)) }
}

enum StsCheckResult {
    Valid { account: String, arn: String },
    Expired { error: String },
    Error { error: String },
}

async fn check_sts_identity(profile: &str) -> StsCheckResult {
    let output = Command::new("aws")
        .args(["sts", "get-caller-identity", "--profile", profile, "--output", "json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .output()
        .await;

    match output {
        Ok(out) => {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // Parse JSON: {"UserId":"...","Account":"123456","Arn":"arn:aws:..."}
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    let account = parsed
                        .get("Account")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let arn = parsed
                        .get("Arn")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    StsCheckResult::Valid { account, arn }
                } else {
                    StsCheckResult::Valid {
                        account: "ok".to_string(),
                        arn: stdout.trim().to_string(),
                    }
                }
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                // Check if it's a token expiry error
                let stderr_lower = stderr.to_lowercase();
                if stderr_lower.contains("expired")
                    || stderr_lower.contains("not authorized")
                    || stderr_lower.contains("invalid")
                    || stderr_lower.contains("the sso session")
                    || stderr_lower.contains("token has expired")
                    || stderr_lower.contains("refresh failed")
                {
                    StsCheckResult::Expired { error: stderr }
                } else {
                    StsCheckResult::Error { error: stderr }
                }
            }
        }
        Err(e) => StsCheckResult::Error {
            error: format!("Failed to run aws cli: {}", e),
        },
    }
}
