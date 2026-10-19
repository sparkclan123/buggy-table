use tauri::{
    App, Emitter,
    menu::{MenuBuilder, MenuItem, SubmenuBuilder},
};

pub fn install(app: &App) -> tauri::Result<()> {
    // Native macOS Close Window consumes Cmd+W before a webview sees it.
    // Own that accelerator and let the workspace apply its unsaved-change guard.
    let close = MenuItem::with_id(
        app,
        "close-table-tab",
        "Close table tab",
        true,
        Some("CmdOrCtrl+W"),
    )?;
    let file = SubmenuBuilder::new(app, "File").item(&close);
    #[cfg(not(target_os = "macos"))]
    let file = file.separator().quit();
    let file = file.build()?;
    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .build()?;
    let menu = MenuBuilder::new(app);
    #[cfg(target_os = "macos")]
    let menu = menu.item(
        &SubmenuBuilder::new(app, "buggy-table")
            .about(None)
            .separator()
            .services()
            .separator()
            .hide()
            .hide_others()
            .separator()
            .quit()
            .build()?,
    );
    app.set_menu(menu.item(&file).item(&edit).item(&window).build()?)?;
    app.on_menu_event(|app, event| {
        if event.id().as_ref() == "close-table-tab" {
            let _ = app.emit("close-table-tab", ());
        }
    });
    Ok(())
}
