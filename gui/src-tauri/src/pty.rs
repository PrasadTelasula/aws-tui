use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

pub struct PtySession {
    writer: Box<dyn Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

#[derive(Default)]
pub struct PtyManager {
    sessions: Mutex<HashMap<String, PtySession>>,
}

fn data_event(id: &str) -> String {
    format!("pty://{id}/data")
}

fn exit_event(id: &str) -> String {
    format!("pty://{id}/exit")
}

#[tauri::command]
pub async fn pty_open(
    id: String,
    shell: Option<String>,
    cwd: Option<String>,
    rows: Option<u16>,
    cols: Option<u16>,
    profile: Option<String>,
    region: Option<String>,
    app: AppHandle,
    state: State<'_, PtyManager>,
) -> Result<(), String> {
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: rows.unwrap_or(30),
            cols: cols.unwrap_or(100),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    let shell_cmd = shell
        .or_else(|| std::env::var("SHELL").ok())
        .unwrap_or_else(|| "/bin/bash".into());

    let mut cmd = CommandBuilder::new(&shell_cmd);
    cmd.arg("-l");
    if let Some(c) = cwd {
        cmd.cwd(c);
    } else if let Some(home) = dirs::home_dir() {
        cmd.cwd(home);
    }
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    if let Some(p) = profile {
        cmd.env("AWS_PROFILE", p);
    }
    if let Some(r) = region {
        cmd.env("AWS_REGION", r.clone());
        cmd.env("AWS_DEFAULT_REGION", r);
    }

    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);

    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;

    let id_for_thread = id.clone();
    let app_for_thread = app.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buf[..n]).into_owned();
                    if app_for_thread.emit(&data_event(&id_for_thread), chunk).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = app_for_thread.emit(&exit_event(&id_for_thread), ());
    });

    state.sessions.lock().unwrap().insert(
        id,
        PtySession {
            writer,
            master: pair.master,
            _child: child,
        },
    );
    Ok(())
}

#[tauri::command]
pub async fn pty_write(
    id: String,
    data: String,
    state: State<'_, PtyManager>,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().unwrap();
    let s = sessions
        .get_mut(&id)
        .ok_or_else(|| format!("pty {id} not found"))?;
    s.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    s.writer.flush().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn pty_resize(
    id: String,
    rows: u16,
    cols: u16,
    state: State<'_, PtyManager>,
) -> Result<(), String> {
    let sessions = state.sessions.lock().unwrap();
    let s = sessions
        .get(&id)
        .ok_or_else(|| format!("pty {id} not found"))?;
    s.master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn pty_close(id: String, state: State<'_, PtyManager>) -> Result<(), String> {
    state.sessions.lock().unwrap().remove(&id);
    Ok(())
}
