pub mod models;
mod mutations;
pub mod policy;
mod postgres;
pub mod service;

#[cfg(feature = "desktop")]
mod commands;

#[cfg(feature = "desktop")]
pub fn run() {
    tauri::Builder::default()
        .manage(service::Workbench::default())
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::disconnect,
            commands::run_query,
            commands::cancel_query,
            commands::browse_table,
            commands::inspect_table,
            commands::get_overview,
            commands::save_table_changes,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start desktop application");
}
