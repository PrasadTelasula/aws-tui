use regex::Regex;
use std::collections::HashMap;
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
    Error(String),
}

impl SessionStatus {

}

#[derive(Debug)]
pub struct Session {
    _alias_name: String,
    _command: String,
    pub status: SessionStatus,
    pub pid: Option<u32>,
    pub output_lines: Vec<String>,
    child: Option<Child>,
}

impl Session {
    pub fn new(alias_name: String, command: String) -> Self {
        Self {
            _alias_name: alias_name,
            _command: command,
            status: SessionStatus::Stopped,
            pid: None,
            output_lines: Vec::new(),
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
        output_tx: tokio::sync::mpsc::UnboundedSender<(String, String)>,
    ) -> Result<(), String> {
        // If session exists and is running, don't start again
        if let Some(session) = self.sessions.get(alias_name) {
            let s = session.lock().await;
            if s.status == SessionStatus::Running || s.status == SessionStatus::Starting {
                return Err("Session already running".to_string());
            }
        }

        let session = Arc::new(Mutex::new(Session::new(
            alias_name.to_string(),
            command.to_string(),
        )));

        {
            let mut s = session.lock().await;
            s.status = SessionStatus::Starting;
            s.output_lines.clear();
            s.output_lines
                .push(format!(">>> Starting: {}", command));

            // Parse the command and spawn it via shell
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

            // Capture stdout
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            s.child = Some(child);
            s.status = SessionStatus::Running;

            let name_clone = alias_name.to_string();
            let tx_clone = output_tx.clone();

            // Spawn stdout reader
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

            // Spawn stderr reader
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

        // Spawn a task to monitor the process exit
        let session_clone = session.clone();
        let name_for_monitor = alias_name.to_string();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let mut s = session_clone.lock().await;
                if let Some(ref mut child) = s.child {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            if status.success() {
                                s.status = SessionStatus::Stopped;
                                s.output_lines.push(format!(
                                    ">>> Process exited successfully"
                                ));
                            } else {
                                let msg = format!(
                                    "Process exited with code: {}",
                                    status.code().unwrap_or(-1)
                                );
                                s.status = SessionStatus::Error(msg.clone());
                                s.output_lines.push(format!(">>> {}", msg));
                            }
                            s.child = None;
                            s.pid = None;
                            break;
                        }
                        Ok(None) => {
                            // Still running
                        }
                        Err(e) => {
                            s.status =
                                SessionStatus::Error(format!("Monitor error: {}", e));
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
            let _ = name_for_monitor; // keep alive
        });

        self.sessions.insert(alias_name.to_string(), session);
        Ok(())
    }

    pub async fn stop_session(&mut self, alias_name: &str) -> Result<(), String> {
        if let Some(session) = self.sessions.get(alias_name) {
            let mut s = session.lock().await;
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
                Err("No running process".to_string())
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
            // Keep last 500 lines
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
