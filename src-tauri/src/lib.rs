mod bounded_stream;
pub mod models;
mod mutations;
mod mysql;
pub mod policy;
mod postgres;
mod redis_store;
mod remote_profile;
pub mod service;

#[cfg(feature = "desktop")]
mod commands;
#[cfg(feature = "desktop")]
mod updates;

#[cfg(feature = "desktop")]
pub fn run() {
    tauri::Builder::default()
        .manage(service::Workbench::default())
        .manage(updates::Updates::default())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::disconnect,
            commands::run_query,
            commands::cancel_query,
            commands::browse_table,
            commands::inspect_table,
            commands::get_overview,
            commands::save_table_changes,
            commands::redis_command,
            updates::check_update,
            updates::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start desktop application");
}
