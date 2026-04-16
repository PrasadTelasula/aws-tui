mod commands;
mod config;
mod model;
mod parser;
mod pty;
mod sessions;

use config::AppState;
use pty::PtyManager;
use sessions::SessionManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial = config::load();
    let state = AppState {
        config: std::sync::Mutex::new(initial),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(state)
        .manage(PtyManager::default())
        .manage(SessionManager::new())
        .invoke_handler(tauri::generate_handler![
            commands::list_aliases,
            commands::set_aliases_path,
            commands::get_config,
            commands::list_instances,
            commands::describe_instance,
            commands::list_clusters,
            commands::list_services,
            commands::list_tasks,
            commands::list_containers,
            commands::complete_aws_cli,
            commands::aws_whoami,
            sessions::start_session,
            sessions::stop_session,
            sessions::list_sessions,
            sessions::session_output,
            pty::pty_open,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
