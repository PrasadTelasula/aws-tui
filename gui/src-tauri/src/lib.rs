mod commands;
mod config;
mod model;
mod parser;

use config::AppState;

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
        .invoke_handler(tauri::generate_handler![
            commands::list_aliases,
            commands::set_aliases_path,
            commands::get_config,
            commands::start_session,
            commands::stop_session,
            commands::list_sessions,
            commands::list_instances,
            commands::describe_instance,
            commands::list_clusters,
            commands::list_services,
            commands::list_tasks,
            commands::list_containers,
            commands::complete_aws_cli,
            commands::aws_whoami,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
