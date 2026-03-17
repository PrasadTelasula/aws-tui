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
        // Replace the bare JSON with a single-quoted version
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
    Connected,              // SSO: login succeeded, token is valid
    Expired,                // SSO: token has expired
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
    child: Option<Child>,
}

impl Session {
    pub fn new(kind: SessionKind) -> Self {
        Self {
            status: SessionStatus::Stopped,
            pid: None,
            output_lines: Vec::new(),
            _kind: kind,
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
        // If session exists and is running/connected, don't start again
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
            s.output_lines.push(format!(">>> Starting: {}", command));

            // Properly quote JSON parameters for shell execution
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

        // Monitor process exit
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
                                // SSO: process exits on success, but session is alive
                                if let SessionKind::SsoLogin { ref session_name } = kind_clone {
                                    s.status = SessionStatus::Connected;
                                    s.output_lines.push(format!(
                                        ">>> SSO login succeeded (session: {})",
                                        session_name
                                    ));
                                    s.output_lines.push(
                                        ">>> Token cached — monitoring expiry".to_string(),
                                    );
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
                        Ok(None) => {} // still running
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

            // If it's a connected SSO session (no child process), just mark stopped
            if matches!(s.status, SessionStatus::Connected) {
                s.status = SessionStatus::Stopped;
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
                // No child but might be in an error/expired state — just reset
                s.status = SessionStatus::Stopped;
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
// After SSO login succeeds, periodically check ~/.aws/sso/cache/ for
// token expiry. When the token expires, update status to Expired.

async fn sso_token_watcher(session: Arc<Mutex<Session>>, session_name: String) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

        let status = {
            let s = session.lock().await;
            s.status.clone()
        };

        // Only keep watching if Connected
        if status != SessionStatus::Connected {
            break;
        }

        match check_sso_token(&session_name) {
            SsoTokenState::Valid { expires_in } => {
                let mut s = session.lock().await;
                if expires_in.as_secs() < 300 {
                    // Less than 5 minutes left
                    let mins = expires_in.as_secs() / 60;
                    s.output_lines.push(format!(
                        ">>> Warning: token expires in {} min",
                        mins
                    ));
                }
            }
            SsoTokenState::Expired => {
                let mut s = session.lock().await;
                s.status = SessionStatus::Expired;
                s.output_lines
                    .push(">>> SSO token has expired — re-login required".to_string());
                break;
            }
            SsoTokenState::NotFound => {
                // Cache file not found — might be a different cache structure
                // Keep status as Connected, don't break
            }
        }
    }
}

enum SsoTokenState {
    Valid { expires_in: std::time::Duration },
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

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Check if this cache file belongs to our SSO session
        // AWS SSO cache files contain startUrl or session name info
        let content_lower = content.to_lowercase();
        let is_match = content_lower.contains(&session_lower)
            || content_lower.contains("accesstoken");

        if !is_match {
            continue;
        }

        // Parse expiresAt
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(expires_str) = parsed.get("expiresAt").and_then(|v| v.as_str()) {
                // Parse ISO 8601: "2024-01-15T12:00:00UTC" or "2024-01-15T12:00:00Z"
                if let Some(expires_at) = parse_iso8601(expires_str) {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    if expires_at > now {
                        return SsoTokenState::Valid {
                            expires_in: std::time::Duration::from_secs(expires_at - now),
                        };
                    } else {
                        return SsoTokenState::Expired;
                    }
                }
            }
        }
    }

    SsoTokenState::NotFound
}

/// Simple ISO 8601 parser — returns Unix timestamp
fn parse_iso8601(s: &str) -> Option<u64> {
    // Handles: "2024-01-15T12:30:00Z", "2024-01-15T12:30:00UTC"
    let s = s.trim().replace("UTC", "Z").replace("utc", "Z");
    let s = s.trim_end_matches('Z');

    let parts: Vec<&str> = s.split('T').collect();
    if parts.len() != 2 {
        return None;
    }

    let date_parts: Vec<u64> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
    let time_str = parts[1].split('+').next().unwrap_or(parts[1]); // strip timezone offset
    let time_parts: Vec<u64> = time_str.split(':').filter_map(|p| p.parse().ok()).collect();

    if date_parts.len() != 3 || time_parts.len() < 2 {
        return None;
    }

    let (year, month, day) = (date_parts[0], date_parts[1], date_parts[2]);
    let (hour, minute) = (time_parts[0], time_parts[1]);
    let second = time_parts.get(2).copied().unwrap_or(0);

    // Simplified days-from-epoch calculation
    let mut days: u64 = 0;
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
    }
    let month_days = [31, 28 + if is_leap(year) { 1 } else { 0 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 0..(month as usize - 1).min(11) {
        days += month_days[m];
    }
    days += day - 1;

    Some(days * 86400 + hour * 3600 + minute * 60 + second)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
