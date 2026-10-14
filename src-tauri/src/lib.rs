mod bounded_stream;
pub mod models;
mod mutations;
mod mysql;
pub mod policy;
mod postgres;
pub mod profiles;
mod redis_store;
mod remote_profile;
pub mod service;

#[cfg(feature = "desktop")]
mod commands;
#[cfg(feature = "desktop")]
mod connection_commands;
#[cfg(feature = "desktop")]
mod updates;

#[cfg(feature = "desktop")]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            use tauri::Manager;
            app.manage(profiles::Profiles::new(app.path().app_local_data_dir()?));
            Ok(())
        })
        .manage(service::Workbench::default())
        .manage(updates::Updates::default())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            connection_commands::list_connections,
            connection_commands::connect_profile,
            connection_commands::reconnect_profile,
            connection_commands::update_connection,
            connection_commands::remove_connection,
            connection_commands::list_schemas,
            connection_commands::schema_tables,
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
