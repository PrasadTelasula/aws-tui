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
    IamProfile { profile_name: String },
    Other,
}

/// Resolved temporary credentials from `aws configure export-credentials`.
#[derive(Debug, Clone)]
pub struct CredentialInfo {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: String,
    /// Raw expiration string exactly as returned by the AWS CLI.
    pub expiration: String,
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
    pub credentials: Option<CredentialInfo>,
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
            credentials: None,
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

    pub async fn get_credentials(&self, alias_name: &str) -> Option<CredentialInfo> {
        if let Some(session) = self.sessions.get(alias_name) {
            let s = session.lock().await;
            s.credentials.clone()
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

                    // Seed credentials immediately on startup
                    if let Some(c) = fetch_credentials(&profile).await {
                        let mut s = session.lock().await;
                        s.credentials = Some(c);
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

    /// On startup: check which IAM profiles in `~/.aws/credentials` are valid.
    /// For each alias whose group type is IAM, verify the profile exists in the
    /// credentials file and that `sts get-caller-identity` succeeds.
    pub async fn check_existing_iam_profiles(
        &mut self,
        aliases: &[(String, String)], // (alias_name, profile_name)
    ) {
        for (alias_name, profile_name) in aliases {
            if !iam_profile_exists_in_credentials(profile_name) {
                continue;
            }

            match check_sts_identity(profile_name).await {
                StsCheckResult::Valid { account, arn } => {
                    let kind = SessionKind::IamProfile {
                        profile_name: profile_name.clone(),
                    };
                    let session = Arc::new(Mutex::new(Session::new(kind)));

                    {
                        let mut s = session.lock().await;
                        s.status = SessionStatus::Connected;
                        s.sso_profile = Some(profile_name.clone());
                        s.output_lines.push(format!(
                            ">>> IAM profile '{}' found in ~/.aws/credentials",
                            profile_name
                        ));
                        s.output_lines.push(format!(
                            ">>> Credentials verified — {} ({})",
                            arn, account
                        ));
                    }

                    // Read directly from ~/.aws/credentials (no shell-out needed)
                    if let Some(c) = read_iam_credentials_from_file(profile_name) {
                        let mut s = session.lock().await;
                        s.credentials = Some(c);
                    }

                    self.sessions.insert(alias_name.clone(), session);
                }
                StsCheckResult::Expired { error } | StsCheckResult::Error { error } => {
                    // Profile exists but credentials are invalid — register as Error so it's visible
                    let kind = SessionKind::IamProfile {
                        profile_name: profile_name.clone(),
                    };
                    let session = Arc::new(Mutex::new(Session::new(kind)));
                    {
                        let mut s = session.lock().await;
                        s.status = SessionStatus::Error(error.clone());
                        s.output_lines.push(format!(
                            ">>> IAM profile '{}' — credential check failed: {}",
                            profile_name, error
                        ));
                    }
                    self.sessions.insert(alias_name.clone(), session);
                }
            }
        }
    }

    /// Re-verify an IAM profile on demand (e.g. when user presses Enter on it).
    /// Refreshes credentials and updates session state.
    pub async fn connect_iam_profile(
        &mut self,
        alias_name: &str,
        profile_name: &str,
    ) -> Result<(), String> {
        if !iam_profile_exists_in_credentials(profile_name) {
            return Err(format!(
                "Profile '{}' not found in ~/.aws/credentials",
                profile_name
            ));
        }

        match check_sts_identity(profile_name).await {
            StsCheckResult::Valid { account, arn } => {
                let kind = SessionKind::IamProfile {
                    profile_name: profile_name.to_string(),
                };
                let session = Arc::new(Mutex::new(Session::new(kind)));

                {
                    let mut s = session.lock().await;
                    s.status = SessionStatus::Connected;
                    s.sso_profile = Some(profile_name.to_string());
                    s.output_lines.push(format!(
                        ">>> IAM credentials verified — {} ({})",
                        arn, account
                    ));
                }

                // Read directly from ~/.aws/credentials (no shell-out needed)
                if let Some(c) = read_iam_credentials_from_file(profile_name) {
                    let mut s = session.lock().await;
                    s.credentials = Some(c);
                }

                self.sessions.insert(alias_name.to_string(), session);
                Ok(())
            }
            StsCheckResult::Expired { error } | StsCheckResult::Error { error } => {
                Err(format!("Credential verification failed: {}", error))
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

// ─── IAM credentials file check ─────────────────────────────────────
// Checks whether a named profile section exists in ~/.aws/credentials.

fn iam_profile_exists_in_credentials(profile_name: &str) -> bool {
    let creds_path = match dirs::home_dir() {
        Some(h) => h.join(".aws/credentials"),
        None => return false,
    };
    let content = match fs::read_to_string(creds_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let target = format!("[{}]", profile_name);
    content.lines().any(|line| line.trim() == target)
}

// ─── Direct IAM credential reader ───────────────────────────────────
// Reads aws_access_key_id and aws_secret_access_key directly from
// ~/.aws/credentials for a named profile section. Does not shell out.

fn read_iam_credentials_from_file(profile_name: &str) -> Option<CredentialInfo> {
    let creds_path = dirs::home_dir()?.join(".aws/credentials");
    let content = fs::read_to_string(creds_path).ok()?;

    let target_section = format!("[{}]", profile_name);
    let mut in_section = false;
    let mut access_key = String::new();
    let mut secret_key = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_section {
                // Entered next section — stop
                break;
            }
            in_section = trimmed == target_section;
            continue;
        }

        if in_section {
            if let Some((key, value)) = trimmed.split_once('=') {
                match key.trim().to_lowercase().as_str() {
                    "aws_access_key_id" => access_key = value.trim().to_string(),
                    "aws_secret_access_key" => secret_key = value.trim().to_string(),
                    _ => {}
                }
            }
        }
    }

    if access_key.is_empty() || secret_key.is_empty() {
        return None;
    }

    Some(CredentialInfo {
        access_key_id: access_key,
        secret_access_key: secret_key,
        session_token: String::new(),
        expiration: String::new(),
    })
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
        // Fetch full credentials AFTER STS (may trigger token refresh)
        let creds = fetch_credentials(&profile).await;

        {
            let mut s = session.lock().await;

            if s.status != SessionStatus::Connected {
                break;
            }

            match check_result {
                StsCheckResult::Valid { account, arn } => {
                    if let Some(ref c) = creds {
                        s.credentials = Some(c.clone());
                    }
                    s.token_expires_at = Some(format!("{} ({})", arn, account));
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

        tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
    }
}

// ─── Credential Fetcher via AWS CLI ─────────────────────────────────
// Runs `aws configure export-credentials --profile <profile>` and
// captures all credential fields. Profile-scoped — safe for multiple
// concurrent SSO sessions.

async fn fetch_credentials(profile: &str) -> Option<CredentialInfo> {
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

    let access_key_id     = json.get("AccessKeyId").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let secret_access_key = json.get("SecretAccessKey").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let session_token     = json.get("SessionToken").and_then(|v| v.as_str()).unwrap_or("").to_string();
    // Store the raw expiration string exactly as the CLI returned it — no calculation.
    let expiration        = json.get("Expiration").and_then(|v| v.as_str()).unwrap_or("").to_string();

    Some(CredentialInfo { access_key_id, secret_access_key, session_token, expiration })
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
