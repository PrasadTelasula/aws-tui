use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
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
    /// For SSO: when the token expires (human-readable) and remaining seconds
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
            s.output_lines.push(format!(">>> Starting: {}", command));

            let shell_command = quote_parameters_for_shell(command);
            let mut child = Command::new("sh")
                .arg("-c")
                .arg(&shell_command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
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

                                    // Immediately check token expiry
                                    match check_sso_token(session_name) {
                                        SsoTokenState::Valid { expires_in, expires_at_str } => {
                                            s.token_expires_at = Some(expires_at_str.clone());
                                            s.token_remaining_secs = Some(expires_in.as_secs());
                                            let remaining = format_duration(expires_in.as_secs());
                                            s.output_lines.push(format!(
                                                ">>> Token expires at {} ({} remaining)",
                                                expires_at_str, remaining
                                            ));
                                        }
                                        _ => {
                                            s.output_lines.push(
                                                ">>> Token cached (could not read expiry from cache)".to_string(),
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

            // For SSO sessions, start a token expiry watcher
            if let SessionKind::SsoLogin { ref session_name } = kind_clone {
                let sn = session_name.clone();
                let sess = session_clone.clone();
                tokio::spawn(async move {
                    sso_token_watcher(sess, sn).await;
                });
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

    /// Returns (expires_at_str, remaining_seconds) for SSO sessions
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
}

// ─── SSO Token Cache Watcher ────────────────────────────────────────

async fn sso_token_watcher(session: Arc<Mutex<Session>>, session_name: String) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

        let status = {
            let s = session.lock().await;
            s.status.clone()
        };

        if status != SessionStatus::Connected {
            break;
        }

        match check_sso_token(&session_name) {
            SsoTokenState::Valid { expires_in, expires_at_str } => {
                let mut s = session.lock().await;
                s.token_remaining_secs = Some(expires_in.as_secs());
                s.token_expires_at = Some(expires_at_str);

                if expires_in.as_secs() < 300 {
                    let remaining = format_duration(expires_in.as_secs());
                    s.output_lines.push(format!(
                        ">>> Warning: token expires in {}",
                        remaining
                    ));
                }
            }
            SsoTokenState::Expired => {
                let mut s = session.lock().await;
                s.status = SessionStatus::Expired;
                s.token_remaining_secs = Some(0);
                s.output_lines
                    .push(">>> SSO token has expired — re-login required".to_string());
                break;
            }
            SsoTokenState::NotFound => {
                // Cache file not found — keep Connected, don't falsely expire
            }
        }
    }
}

enum SsoTokenState {
    Valid {
        expires_in: std::time::Duration,
        expires_at_str: String,
    },
    Expired,
    NotFound,
}

fn check_sso_token(session_name: &str) -> SsoTokenState {
    let cache_dir = dirs::home_dir()
        .map(|h| h.join(".aws/sso/cache"))
        .unwrap_or_else(|| PathBuf::from("/tmp"));

    let entries = match fs::read_dir(&cache_dir) {
        Ok(e) => e,
        Err(_) => return SsoTokenState::NotFound,
    };

    let session_lower = session_name.to_lowercase();

    // Collect ALL matching tokens, pick the one with the latest expiry
    let mut best_valid: Option<(u64, String)> = None; // (expires_at_unix, expires_at_str)
    let mut found_any = false;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Only match files that contain our specific session name
        let content_lower = content.to_lowercase();
        if !content_lower.contains(&session_lower) {
            continue;
        }

        let parsed = match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Must have accessToken to be a valid SSO token file
        if parsed.get("accessToken").is_none() {
            continue;
        }

        let expires_str = match parsed.get("expiresAt").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };

        found_any = true;

        if let Some(expires_at) = parse_iso8601(expires_str) {
            if expires_at > now {
                // Valid token — keep the one with latest expiry
                match best_valid {
                    Some((best_ts, _)) if expires_at > best_ts => {
                        best_valid = Some((expires_at, expires_str.to_string()));
                    }
                    None => {
                        best_valid = Some((expires_at, expires_str.to_string()));
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some((expires_at, expires_at_str)) = best_valid {
        SsoTokenState::Valid {
            expires_in: std::time::Duration::from_secs(expires_at - now),
            expires_at_str,
        }
    } else if found_any {
        // Found matching files but all expired
        SsoTokenState::Expired
    } else {
        SsoTokenState::NotFound
    }
}

/// Format seconds into human-readable: "2h 15m", "45m 30s", "30s"
fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let s = secs % 60;
    if hours > 0 {
        format!("{}h {:02}m", hours, mins)
    } else if mins > 0 {
        format!("{}m {:02}s", mins, s)
    } else {
        format!("{}s", s)
    }
}

/// Simple ISO 8601 parser — returns Unix timestamp
fn parse_iso8601(s: &str) -> Option<u64> {
    let s = s.trim().replace("UTC", "Z").replace("utc", "Z");
    let s = s.trim_end_matches('Z');

    let parts: Vec<&str> = s.split('T').collect();
    if parts.len() != 2 {
        return None;
    }

    let date_parts: Vec<u64> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
    let time_str = parts[1].split('+').next().unwrap_or(parts[1]);
    let time_parts: Vec<u64> = time_str.split(':').filter_map(|p| p.parse().ok()).collect();

    if date_parts.len() != 3 || time_parts.len() < 2 {
        return None;
    }

    let (year, month, day) = (date_parts[0], date_parts[1], date_parts[2]);
    let (hour, minute) = (time_parts[0], time_parts[1]);
    let second = time_parts.get(2).copied().unwrap_or(0);

    let mut days: u64 = 0;
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
    }
    let month_days = [
        31,
        28 + if is_leap(year) { 1 } else { 0 },
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    for m in 0..(month as usize - 1).min(11) {
        days += month_days[m];
    }
    days += day - 1;

    Some(days * 86400 + hour * 3600 + minute * 60 + second)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
