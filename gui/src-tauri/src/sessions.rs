use crate::aws::{
    check_sts_identity, fetch_credentials, iam_profile_exists_in_credentials,
    quote_parameters_for_shell, read_iam_credentials_from_file,
    resolve_profile_for_sso_session, resolve_tag_target_in_command, StsCheckResult,
};
use crate::model::{AliasKind, CredentialInfo, SessionState, SessionStatus};
use std::collections::{HashMap, VecDeque};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

const OUTPUT_BUFFER_LINES: usize = 500;
const LIVENESS_INTERVAL_SECS: u64 = 300;
const LIVENESS_INITIAL_DELAY_SECS: u64 = 5;

#[derive(Clone, Debug)]
pub enum SessionKindInternal {
    SsoLogin { session_name: String },
    SsmSession,
    IamProfile { profile_name: String },
    Other,
}

impl SessionKindInternal {
    fn from_alias(kind: &AliasKind, sso_session: Option<&str>, name: &str) -> Self {
        match kind {
            AliasKind::SsoLogin => Self::SsoLogin {
                session_name: sso_session.unwrap_or(name).to_string(),
            },
            AliasKind::SsmSession => Self::SsmSession,
            AliasKind::IamProfile => Self::IamProfile {
                profile_name: name.to_string(),
            },
            AliasKind::Other => Self::Other,
        }
    }
}

pub struct ActiveSession {
    pub status: SessionStatus,
    #[allow(dead_code)]
    pub kind: SessionKindInternal,
    pub credentials: Option<CredentialInfo>,
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
}

fn output_event(alias: &str) -> String {
    format!("session://{alias}/output")
}

fn status_event(alias: &str) -> String {
    format!("session://{alias}/status")
}

fn emit_status(app: &AppHandle, status: &SessionStatus) {
    let _ = app.emit(&status_event(&status.alias), status);
    let _ = app.emit("sessions://changed", ());
}

fn push_line(map: &SessionMap, app: &AppHandle, alias: &str, line: String) {
    let _ = app.emit(&output_event(alias), &line);
    if let Ok(mut sessions) = map.lock() {
        if let Some(s) = sessions.get_mut(alias) {
            if s.output.len() >= OUTPUT_BUFFER_LINES {
                s.output.pop_front();
            }
            s.output.push_back(line);
        }
    }
}

fn snapshot_status(map: &SessionMap, alias: &str) -> Option<SessionStatus> {
    map.lock().ok()?.get(alias).map(|s| s.status.clone())
}

fn spawn_reader<R>(
    reader: R,
    alias: String,
    app: AppHandle,
    map: Arc<SessionMap>,
    is_stderr: bool,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let formatted = if is_stderr {
                format!("[stderr] {line}")
            } else {
                line
            };
            push_line(&map, &app, &alias, formatted);
        }
    });
}

#[tauri::command]
pub async fn start_session(
    alias: String,
    command: String,
    kind: AliasKind,
    sso_session_name: Option<String>,
    profile_name: Option<String>,
    app: AppHandle,
    state: State<'_, SessionManager>,
) -> Result<SessionStatus, String> {
    {
        if let Some(existing) = state.sessions.lock().unwrap().get(&alias) {
            if matches!(
                existing.status.state,
                SessionState::Running | SessionState::Starting | SessionState::Connected
            ) {
                return Ok(existing.status.clone());
            }
        }
    }

    let internal_kind = SessionKindInternal::from_alias(
        &kind,
        sso_session_name.as_deref(),
        profile_name.as_deref().unwrap_or(&alias),
    );

    // For IAM profiles: connect-only, no spawn
    if let SessionKindInternal::IamProfile { profile_name } = &internal_kind {
        return connect_iam_profile(alias, profile_name.clone(), app.clone(), state.inner().clone()).await;
    }

    let starting = SessionStatus {
        alias: alias.clone(),
        state: SessionState::Starting,
        pid: None,
        started_at: Some(chrono::Utc::now().to_rfc3339()),
        error_message: None,
        sso_profile: None,
        identity_arn: None,
        identity_account: None,
        token_expires_at: None,
        token_remaining_secs: None,
        has_credentials: false,
    };
    state.sessions.lock().unwrap().insert(
        alias.clone(),
        ActiveSession {
            status: starting.clone(),
            kind: internal_kind.clone(),
            credentials: None,
            child: None,
            output: VecDeque::with_capacity(OUTPUT_BUFFER_LINES),
        },
    );
    emit_status(&app, &starting);
    push_line(
        &state.sessions,
        &app,
        &alias,
        format!(">>> Starting: {command}"),
    );

    // Resolve tag-based --target before spawning (SSM only)
    let resolved_command = match &internal_kind {
        SessionKindInternal::SsmSession => match resolve_tag_target_in_command(&command).await {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("Tag resolution failed: {e}");
                fail_session(&state.sessions, &app, &alias, &msg);
                return Err(msg);
            }
        },
        _ => command.clone(),
    };
    let shell_command = quote_parameters_for_shell(&resolved_command);

    let stdin_cfg = match &internal_kind {
        SessionKindInternal::SsmSession => Stdio::piped(),
        _ => Stdio::null(),
    };

    let mut child = match Command::new("sh")
        .arg("-c")
        .arg(&shell_command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(stdin_cfg)
        .kill_on_drop(true)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Failed to spawn: {e}");
            fail_session(&state.sessions, &app, &alias, &msg);
            return Err(msg);
        }
    };

    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let running = SessionStatus {
        state: SessionState::Running,
        pid,
        ..starting
    };
    {
        let mut sessions = state.sessions.lock().unwrap();
        if let Some(s) = sessions.get_mut(&alias) {
            s.status = running.clone();
            s.child = Some(child);
        }
    }
    emit_status(&app, &running);

    if let Some(out) = stdout {
        spawn_reader(out, alias.clone(), app.clone(), state.sessions.clone(), false);
    }
    if let Some(err) = stderr {
        spawn_reader(err, alias.clone(), app.clone(), state.sessions.clone(), true);
    }

    let map = state.sessions.clone();
    let alias_for_wait = alias.clone();
    let app_for_wait = app.clone();
    let kind_for_wait = internal_kind.clone();
    tokio::spawn(async move {
        watch_child_exit(map, app_for_wait, alias_for_wait, kind_for_wait).await;
    });

    Ok(running)
}

async fn watch_child_exit(
    map: Arc<SessionMap>,
    app: AppHandle,
    alias: String,
    kind: SessionKindInternal,
) {
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let exit_opt = {
            let mut sessions = match map.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            let s = match sessions.get_mut(&alias) {
                Some(s) => s,
                None => return,
            };
            if let Some(ref mut child) = s.child {
                match child.try_wait() {
                    Ok(Some(status)) => Some(status),
                    Ok(None) => None,
                    Err(e) => {
                        s.status.state = SessionState::Error;
                        s.status.error_message = Some(format!("Monitor error: {e}"));
                        let snap = s.status.clone();
                        drop(sessions);
                        emit_status(&app, &snap);
                        return;
                    }
                }
            } else {
                return;
            }
        };
        if let Some(exit_status) = exit_opt {
            handle_child_exit(&map, &app, &alias, &kind, exit_status).await;
            break;
        }
    }
}

async fn handle_child_exit(
    map: &Arc<SessionMap>,
    app: &AppHandle,
    alias: &str,
    kind: &SessionKindInternal,
    exit_status: std::process::ExitStatus,
) {
    let success = exit_status.success();
    let mut start_liveness_for: Option<String> = None;

    {
        let mut sessions = map.lock().unwrap();
        let s = match sessions.get_mut(alias) {
            Some(s) => s,
            None => return,
        };
        s.child = None;
        s.status.pid = None;

        if success {
            match kind {
                SessionKindInternal::SsoLogin { session_name } => {
                    s.status.state = SessionState::Connected;
                    s.output.push_back(format!(
                        ">>> SSO login succeeded (session: {session_name})"
                    ));
                    if let Some(p) = resolve_profile_for_sso_session(session_name) {
                        s.status.sso_profile = Some(p.clone());
                        s.output.push_back(format!(
                            ">>> Monitoring via: aws sts get-caller-identity --profile {p}"
                        ));
                        start_liveness_for = Some(p);
                    } else {
                        s.output.push_back(
                            ">>> No profile found for this sso-session in ~/.aws/config".into(),
                        );
                    }
                }
                _ => {
                    s.status.state = SessionState::Stopped;
                    s.output
                        .push_back(">>> Process exited successfully".to_string());
                }
            }
        } else {
            let msg = format!(
                "Process exited with code: {}",
                exit_status.code().unwrap_or(-1)
            );
            s.status.state = SessionState::Error;
            s.status.error_message = Some(msg.clone());
            s.output.push_back(format!(">>> {msg}"));
        }
    }

    let snap = snapshot_status(map, alias).unwrap_or_else(|| SessionStatus::stopped(alias));
    emit_status(app, &snap);

    if let Some(profile) = start_liveness_for {
        let map = map.clone();
        let app = app.clone();
        let alias = alias.to_string();
        tokio::spawn(async move {
            sso_liveness_watcher(map, app, alias, profile).await;
        });
    }
}

fn fail_session(map: &Arc<SessionMap>, app: &AppHandle, alias: &str, msg: &str) {
    {
        let mut sessions = map.lock().unwrap();
        if let Some(s) = sessions.get_mut(alias) {
            s.status.state = SessionState::Error;
            s.status.error_message = Some(msg.to_string());
            s.output.push_back(format!(">>> {msg}"));
        }
    }
    let snap = snapshot_status(map, alias).unwrap_or_else(|| SessionStatus::stopped(alias));
    emit_status(app, &snap);
}

async fn sso_liveness_watcher(
    map: Arc<SessionMap>,
    app: AppHandle,
    alias: String,
    profile: String,
) {
    tokio::time::sleep(Duration::from_secs(LIVENESS_INITIAL_DELAY_SECS)).await;

    loop {
        let still_connected = matches!(
            snapshot_status(&map, &alias).map(|s| s.state),
            Some(SessionState::Connected)
        );
        if !still_connected {
            break;
        }

        let check = check_sts_identity(&profile).await;
        let creds = fetch_credentials(&profile).await;

        {
            let mut sessions = map.lock().unwrap();
            let s = match sessions.get_mut(&alias) {
                Some(s) => s,
                None => break,
            };
            if s.status.state != SessionState::Connected {
                break;
            }
            match check {
                StsCheckResult::Valid { account, arn } => {
                    if let Some(c) = creds {
                        s.credentials = Some(c.clone());
                        s.status.has_credentials = true;
                        if !c.expiration.is_empty() {
                            s.status.token_expires_at = Some(c.expiration.clone());
                            s.status.token_remaining_secs =
                                seconds_until(&c.expiration);
                        }
                    }
                    s.status.identity_arn = Some(arn.clone());
                    s.status.identity_account = Some(account.clone());
                    s.output
                        .push_back(format!(">>> Session verified — {arn} ({account})"));
                }
                StsCheckResult::Expired { error } => {
                    s.status.state = SessionState::Expired;
                    s.status.token_remaining_secs = Some(0);
                    s.output.push_back(format!(">>> SSO session expired — {error}"));
                    s.output
                        .push_back(">>> Re-login required (press Start)".to_string());
                    let snap = s.status.clone();
                    drop(sessions);
                    emit_status(&app, &snap);
                    break;
                }
                StsCheckResult::Error { error } => {
                    s.output
                        .push_back(format!(">>> STS check failed: {error} (will retry)"));
                }
            }
        }
        let snap = snapshot_status(&map, &alias);
        if let Some(s) = snap {
            emit_status(&app, &s);
        }

        tokio::time::sleep(Duration::from_secs(LIVENESS_INTERVAL_SECS)).await;
    }
}

fn seconds_until(iso: &str) -> Option<u64> {
    let parsed = chrono::DateTime::parse_from_rfc3339(iso).ok()?;
    let diff = parsed.with_timezone(&chrono::Utc) - chrono::Utc::now();
    let secs = diff.num_seconds();
    if secs < 0 { Some(0) } else { Some(secs as u64) }
}

async fn connect_iam_profile(
    alias: String,
    profile_name: String,
    app: AppHandle,
    manager: SessionManager,
) -> Result<SessionStatus, String> {
    if !iam_profile_exists_in_credentials(&profile_name) {
        return Err(format!(
            "Profile '{profile_name}' not found in ~/.aws/credentials"
        ));
    }
    match check_sts_identity(&profile_name).await {
        StsCheckResult::Valid { account, arn } => {
            let creds = read_iam_credentials_from_file(&profile_name);
            let status = SessionStatus {
                alias: alias.clone(),
                state: SessionState::Connected,
                pid: None,
                started_at: Some(chrono::Utc::now().to_rfc3339()),
                error_message: None,
                sso_profile: Some(profile_name.clone()),
                identity_arn: Some(arn.clone()),
                identity_account: Some(account.clone()),
                token_expires_at: None,
                token_remaining_secs: None,
                has_credentials: creds.is_some(),
            };
            let mut output = VecDeque::with_capacity(OUTPUT_BUFFER_LINES);
            output.push_back(format!(
                ">>> IAM profile '{profile_name}' found in ~/.aws/credentials"
            ));
            output.push_back(format!(">>> Credentials verified — {arn} ({account})"));
            manager.sessions.lock().unwrap().insert(
                alias.clone(),
                ActiveSession {
                    status: status.clone(),
                    kind: SessionKindInternal::IamProfile {
                        profile_name: profile_name.clone(),
                    },
                    credentials: creds,
                    child: None,
                    output,
                },
            );
            emit_status(&app, &status);
            Ok(status)
        }
        StsCheckResult::Expired { error } | StsCheckResult::Error { error } => {
            Err(format!("Credential verification failed: {error}"))
        }
    }
}

#[tauri::command]
pub async fn stop_session(
    alias: String,
    app: AppHandle,
    state: State<'_, SessionManager>,
) -> Result<SessionStatus, String> {
    let took_child = {
        let mut sessions = state.sessions.lock().unwrap();
        let s = match sessions.get_mut(&alias) {
            Some(s) => s,
            None => return Err("Session not found".into()),
        };
        if matches!(s.status.state, SessionState::Connected) && s.child.is_none() {
            s.status = SessionStatus::stopped(&alias);
            s.credentials = None;
            s.output.push_back(">>> SSO session dismissed".into());
            let snap = s.status.clone();
            drop(sessions);
            emit_status(&app, &snap);
            return Ok(snap);
        }
        s.child.take()
    };

    if let Some(mut child) = took_child {
        let _ = child.kill().await;
    }

    let final_status = SessionStatus::stopped(&alias);
    {
        let mut sessions = state.sessions.lock().unwrap();
        if let Some(s) = sessions.get_mut(&alias) {
            s.status = final_status.clone();
            s.credentials = None;
            s.output
                .push_back(">>> Session stopped by user".to_string());
        }
    }
    emit_status(&app, &final_status);
    Ok(final_status)
}

#[tauri::command]
pub async fn stop_all_sessions(
    app: AppHandle,
    state: State<'_, SessionManager>,
) -> Result<u32, String> {
    let map = state.sessions.clone();
    let to_stop: Vec<(String, Option<Child>, bool)> = {
        let mut sessions = map.lock().unwrap();
        let mut out = Vec::new();
        let names: Vec<String> = sessions.keys().cloned().collect();
        for name in names {
            if let Some(s) = sessions.get_mut(&name) {
                let active = matches!(
                    s.status.state,
                    SessionState::Running | SessionState::Connected | SessionState::Starting
                );
                if active {
                    let child = s.child.take();
                    let was_dismiss_only =
                        matches!(s.status.state, SessionState::Connected) && child.is_none();
                    out.push((name, child, was_dismiss_only));
                }
            }
        }
        out
    };

    let mut count = 0u32;
    for (name, child, _dismiss) in to_stop {
        if let Some(mut c) = child {
            let _ = c.kill().await;
        }
        let final_status = SessionStatus::stopped(&name);
        {
            let mut sessions = map.lock().unwrap();
            if let Some(s) = sessions.get_mut(&name) {
                s.status = final_status.clone();
                s.credentials = None;
                s.output.push_back(">>> Session stopped (stop-all)".into());
            }
        }
        emit_status(&app, &final_status);
        count += 1;
    }
    Ok(count)
}

#[tauri::command]
pub async fn list_sessions(
    state: State<'_, SessionManager>,
) -> Result<Vec<SessionStatus>, String> {
    Ok(state
        .sessions
        .lock()
        .unwrap()
        .values()
        .map(|s| s.status.clone())
        .collect())
}

#[tauri::command]
pub async fn session_output(
    alias: String,
    state: State<'_, SessionManager>,
) -> Result<Vec<String>, String> {
    Ok(state
        .sessions
        .lock()
        .unwrap()
        .get(&alias)
        .map(|s| s.output.iter().cloned().collect())
        .unwrap_or_default())
}

#[tauri::command]
pub async fn get_credentials(
    alias: String,
    state: State<'_, SessionManager>,
) -> Result<Option<CredentialInfo>, String> {
    Ok(state
        .sessions
        .lock()
        .unwrap()
        .get(&alias)
        .and_then(|s| s.credentials.clone()))
}

/// Tuple of (alias, sso_session_name).
#[tauri::command]
pub async fn check_existing_sso(
    aliases: Vec<(String, String)>,
    app: AppHandle,
    state: State<'_, SessionManager>,
) -> Result<Vec<SessionStatus>, String> {
    let mut found = Vec::new();
    for (alias_name, sso_session_name) in aliases {
        let profile = match resolve_profile_for_sso_session(&sso_session_name) {
            Some(p) => p,
            None => continue,
        };
        match check_sts_identity(&profile).await {
            StsCheckResult::Valid { account, arn } => {
                let creds = fetch_credentials(&profile).await;
                let token_expires_at = creds.as_ref().map(|c| c.expiration.clone()).filter(|s| !s.is_empty());
                let token_remaining_secs = token_expires_at
                    .as_deref()
                    .and_then(seconds_until);
                let status = SessionStatus {
                    alias: alias_name.clone(),
                    state: SessionState::Connected,
                    pid: None,
                    started_at: Some(chrono::Utc::now().to_rfc3339()),
                    error_message: None,
                    sso_profile: Some(profile.clone()),
                    identity_arn: Some(arn.clone()),
                    identity_account: Some(account.clone()),
                    token_expires_at,
                    token_remaining_secs,
                    has_credentials: creds.is_some(),
                };
                let mut output = VecDeque::with_capacity(OUTPUT_BUFFER_LINES);
                output.push_back(format!(
                    ">>> Existing SSO session detected (profile: {profile})"
                ));
                output.push_back(format!(">>> Session verified — {arn} ({account})"));
                state.sessions.lock().unwrap().insert(
                    alias_name.clone(),
                    ActiveSession {
                        status: status.clone(),
                        kind: SessionKindInternal::SsoLogin {
                            session_name: sso_session_name.clone(),
                        },
                        credentials: creds,
                        child: None,
                        output,
                    },
                );
                emit_status(&app, &status);
                found.push(status);

                let map = state.sessions.clone();
                let app_clone = app.clone();
                let alias_for_watch = alias_name.clone();
                let profile_for_watch = profile.clone();
                tokio::spawn(async move {
                    sso_liveness_watcher(map, app_clone, alias_for_watch, profile_for_watch).await;
                });
            }
            _ => {}
        }
    }
    Ok(found)
}

/// Tuple of (alias, profile_name).
#[tauri::command]
pub async fn check_existing_iam(
    aliases: Vec<(String, String)>,
    app: AppHandle,
    state: State<'_, SessionManager>,
) -> Result<Vec<SessionStatus>, String> {
    let mut found = Vec::new();
    for (alias_name, profile_name) in aliases {
        if !iam_profile_exists_in_credentials(&profile_name) {
            continue;
        }
        match check_sts_identity(&profile_name).await {
            StsCheckResult::Valid { account, arn } => {
                let creds = read_iam_credentials_from_file(&profile_name);
                let status = SessionStatus {
                    alias: alias_name.clone(),
                    state: SessionState::Connected,
                    pid: None,
                    started_at: Some(chrono::Utc::now().to_rfc3339()),
                    error_message: None,
                    sso_profile: Some(profile_name.clone()),
                    identity_arn: Some(arn.clone()),
                    identity_account: Some(account.clone()),
                    token_expires_at: None,
                    token_remaining_secs: None,
                    has_credentials: creds.is_some(),
                };
                let mut output = VecDeque::with_capacity(OUTPUT_BUFFER_LINES);
                output.push_back(format!(
                    ">>> IAM profile '{profile_name}' found in ~/.aws/credentials"
                ));
                output.push_back(format!(">>> Credentials verified — {arn} ({account})"));
                state.sessions.lock().unwrap().insert(
                    alias_name.clone(),
                    ActiveSession {
                        status: status.clone(),
                        kind: SessionKindInternal::IamProfile {
                            profile_name: profile_name.clone(),
                        },
                        credentials: creds,
                        child: None,
                        output,
                    },
                );
                emit_status(&app, &status);
                found.push(status);
            }
            StsCheckResult::Expired { error } | StsCheckResult::Error { error } => {
                let status = SessionStatus {
                    alias: alias_name.clone(),
                    state: SessionState::Error,
                    pid: None,
                    started_at: None,
                    error_message: Some(error.clone()),
                    sso_profile: Some(profile_name.clone()),
                    identity_arn: None,
                    identity_account: None,
                    token_expires_at: None,
                    token_remaining_secs: None,
                    has_credentials: false,
                };
                state.sessions.lock().unwrap().insert(
                    alias_name.clone(),
                    ActiveSession {
                        status: status.clone(),
                        kind: SessionKindInternal::IamProfile {
                            profile_name: profile_name.clone(),
                        },
                        credentials: None,
                        child: None,
                        output: VecDeque::new(),
                    },
                );
                emit_status(&app, &status);
            }
        }
    }
    Ok(found)
}

