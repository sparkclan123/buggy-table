fn main() {
    #[cfg(feature = "desktop")]
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "connect",
            "list_connections",
            "connect_profile",
            "reconnect_profile",
            "update_connection",
            "remove_connection",
            "list_schemas",
            "schema_tables",
            "disconnect",
            "run_query",
            "cancel_query",
            "browse_table",
            "count_table_rows",
            "inspect_table",
            "get_overview",
            "save_table_changes",
            "redis_command",
            "check_update",
            "install_update",
        ]),
    ))
    .expect("failed to build Tauri application metadata");
}
