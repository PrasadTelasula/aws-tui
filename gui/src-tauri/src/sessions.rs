use crate::model::{SessionState, SessionStatus};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

const OUTPUT_BUFFER_LINES: usize = 500;

pub struct ActiveSession {
    pub status: SessionStatus,
    pub child: Option<Child>,
    pub output: VecDeque<String>,
}

type SessionMap = Mutex<HashMap<String, ActiveSession>>;

#[derive(Default, Clone)]
pub struct SessionManager {
    sessions: Arc<SessionMap>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    fn snapshot_statuses(&self) -> Vec<SessionStatus> {
        self.sessions
            .lock()
            .unwrap()
            .values()
            .map(|s| s.status.clone())
            .collect()
    }

    fn output_lines(&self, alias: &str) -> Vec<String> {
        self.sessions
            .lock()
            .unwrap()
            .get(alias)
            .map(|s| s.output.iter().cloned().collect())
            .unwrap_or_default()
    }
}

fn output_event(alias: &str) -> String {
    format!("session://{alias}/output")
}

fn status_event(alias: &str) -> String {
    format!("session://{alias}/status")
}

fn push_line(map: &SessionMap, alias: &str, line: &str) {
    if let Ok(mut sessions) = map.lock() {
        if let Some(s) = sessions.get_mut(alias) {
            if s.output.len() >= OUTPUT_BUFFER_LINES {
                s.output.pop_front();
            }
            s.output.push_back(line.to_string());
        }
    }
}

fn mark_idle(map: &SessionMap, alias: &str) {
    if let Ok(mut sessions) = map.lock() {
        if let Some(s) = sessions.get_mut(alias) {
            s.status.state = SessionState::Idle;
            s.status.pid = None;
            s.child = None;
        }
    }
}

fn spawn_reader<R>(reader: R, alias: String, app: AppHandle, map: Arc<SessionMap>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            push_line(&map, &alias, &line);
            let _ = app.emit(&output_event(&alias), &line);
        }
    });
}

#[tauri::command]
pub async fn start_session(
    alias: String,
    command: String,
    app: AppHandle,
    state: State<'_, SessionManager>,
) -> Result<SessionStatus, String> {
    {
        let sessions = state.sessions.lock().unwrap();
        if let Some(existing) = sessions.get(&alias) {
            if matches!(
                existing.status.state,
                SessionState::Active | SessionState::Starting
            ) {
                return Ok(existing.status.clone());
            }
        }
    }

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("failed to spawn: {e}"))?;

    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let status = SessionStatus {
        alias: alias.clone(),
        state: SessionState::Active,
        pid,
        started_at: Some(chrono::Utc::now().to_rfc3339()),
        expires_at: None,
    };

    state.sessions.lock().unwrap().insert(
        alias.clone(),
        ActiveSession {
            status: status.clone(),
            child: Some(child),
            output: VecDeque::with_capacity(OUTPUT_BUFFER_LINES),
        },
    );

    if let Some(out) = stdout {
        spawn_reader(out, alias.clone(), app.clone(), state.sessions.clone());
    }
    if let Some(err) = stderr {
        spawn_reader(err, alias.clone(), app.clone(), state.sessions.clone());
    }

    let map = state.sessions.clone();
    let alias_for_wait = alias.clone();
    let app_for_wait = app.clone();
    tokio::spawn(async move {
        let child_opt = {
            let mut sessions = map.lock().unwrap();
            sessions
                .get_mut(&alias_for_wait)
                .and_then(|s| s.child.take())
        };
        if let Some(mut child) = child_opt {
            let _ = child.wait().await;
        }
        mark_idle(&map, &alias_for_wait);
        let idle = SessionStatus {
            alias: alias_for_wait.clone(),
            state: SessionState::Idle,
            pid: None,
            started_at: None,
            expires_at: None,
        };
        let _ = app_for_wait.emit(&status_event(&alias_for_wait), &idle);
    });

    let _ = app.emit(&status_event(&alias), &status);
    Ok(status)
}

#[tauri::command]
pub async fn stop_session(
    alias: String,
    app: AppHandle,
    state: State<'_, SessionManager>,
) -> Result<SessionStatus, String> {
    if let Some(mut s) = state.sessions.lock().unwrap().remove(&alias) {
        if let Some(mut child) = s.child.take() {
            let _ = child.start_kill();
        }
    }
    let status = SessionStatus {
        alias: alias.clone(),
        state: SessionState::Idle,
        pid: None,
        started_at: None,
        expires_at: None,
    };
    let _ = app.emit(&status_event(&alias), &status);
    Ok(status)
}

#[tauri::command]
pub async fn list_sessions(
    state: State<'_, SessionManager>,
) -> Result<Vec<SessionStatus>, String> {
    Ok(state.snapshot_statuses())
}

#[tauri::command]
pub async fn session_output(
    alias: String,
    state: State<'_, SessionManager>,
) -> Result<Vec<String>, String> {
    Ok(state.output_lines(&alias))
}
