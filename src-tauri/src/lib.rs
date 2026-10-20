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
pub mod vpn;

#[cfg(feature = "desktop")]
mod commands;
#[cfg(feature = "desktop")]
mod connection_commands;
#[cfg(feature = "desktop")]
mod desktop_menu;
#[cfg(feature = "desktop")]
mod updates;
#[cfg(feature = "desktop")]
mod vpn_commands;

#[cfg(feature = "desktop")]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            use tauri::Manager;
            app.manage(profiles::Profiles::new(app.path().app_local_data_dir()?));
            app.manage(vpn::store::VpnStore::new(app.path().app_local_data_dir()?));
            desktop_menu::install(app)?;
            Ok(())
        })
        .manage(service::Workbench::default())
        .manage(vpn::runtime::VpnRuntime::default())
        .manage(updates::Updates::default())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            vpn_commands::vpn_profiles,
            vpn_commands::discard_vpn_draft,
            vpn_commands::save_vpn_profile,
            vpn_commands::remove_vpn_profile,
            vpn_commands::vpn_runtime,
            vpn_commands::prepare_vpn,
            vpn_commands::vpn_status,
            vpn_commands::set_connection_vpn,
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
            commands::count_table_rows,
            commands::inspect_table,
            commands::get_overview,
            commands::save_table_changes,
            commands::redis_command,
            updates::check_update,
            updates::install_update,
            updates::open_download_page,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start desktop application");
}
