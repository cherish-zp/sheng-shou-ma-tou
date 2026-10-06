// System tray. Milestone 1 scope: show window, quit.
// Tunnel-level tray controls are planned for a later milestone.

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Open Pier", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Pier", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::with_id("pier-tray")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("Pier");

    if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png")) {
        builder = builder.icon(icon);
    }

    let tray = builder.build(app)?;

    // Clicking the tray icon (macOS template icon) opens the window.
    let handle = app.clone();
    tray.on_menu_event(move |app, event| match event.id.as_ref() {
        "show" => show_main_window(app),
        "quit" => {
            app.exit(0);
        }
        _ => {}
    });
    let _ = handle;
    Ok(())
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
