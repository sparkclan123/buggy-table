fn main() {
    #[cfg(feature = "desktop")]
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "connect",
            "disconnect",
            "run_query",
            "cancel_query",
            "browse_table",
            "inspect_table",
            "get_overview",
            "save_table_changes",
        ]),
    ))
    .expect("failed to build Tauri application metadata");
}
