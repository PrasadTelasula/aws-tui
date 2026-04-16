mod commands;
mod model;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_aliases,
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
