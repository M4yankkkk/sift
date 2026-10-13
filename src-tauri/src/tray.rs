use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let toggle_i = MenuItem::with_id(app, "toggle", "Open Sift", true, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", "Preferences...", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit Sift", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&toggle_i, &settings_i, &quit_i])?;

    let icon = app.default_window_icon().cloned().ok_or("No default window icon found")?;

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Sift - Neural Desktop Search")
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "toggle" => {
                    crate::ipc::toggle_main_window(app);
                }
                "settings" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                        let _ = window.emit("open-settings", ());
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                crate::ipc::toggle_main_window(app);
            }
        })
        .build(app)?;

    Ok(())
}
