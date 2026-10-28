//! System tray icon and menu.

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Wry,
};

use crate::{commands, popup, updates};

pub struct TrayState {
    update_item: MenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open clipboard", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let update = MenuItem::with_id(app, "update", "Check for updates…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &update,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    app.manage(TrayState { update_item: update });

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Simple Clipboard")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => popup::show(app),
            "settings" => commands::open_settings_window(app, None),
            "update" => on_update_clicked(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                popup::toggle(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn on_update_clicked(app: &AppHandle) {
    commands::open_settings_window(app, Some("updates"));
}

/// Checks for updates in the background and highlights the tray menu entry if one exists.
pub fn check_updates_in_background(app: AppHandle) {
    std::thread::spawn(move || {
        let Ok(info) = updates::check_and_store(&app) else { return };
        if info.available {
            show_update(&app, &info.latest);
        }
    });
}

pub fn show_update(app: &AppHandle, version: &str) {
    let tray = app.state::<TrayState>();
    let _ = tray.update_item.set_text(format!("Install update {version}…"));
    if let Some(icon) = app.tray_by_id("main") {
        let _ = icon.set_tooltip(Some(format!("Simple Clipboard – update {version} available")));
    }
}
