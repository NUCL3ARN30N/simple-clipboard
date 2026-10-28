#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod commands;
mod hotkey;
mod platform;
mod popup;
mod store;
mod tray;
mod updates;

use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::ShortcutState;

use store::AppState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| popup::show(app)))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        popup::toggle(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = AppState::load(store::data_dir(&handle));
            let settings = state.data.lock().unwrap().settings.clone();
            app.manage(state);
            app.manage(updates::PendingUpdate::default());
            updates::cleanup();

            tray::create(&handle)?;

            #[cfg(windows)]
            {
                let h = handle.clone();
                platform::install_hook(move || {
                    let h = h.clone();
                    std::thread::spawn(move || popup::toggle(&h));
                });
            }
            if let Err(e) = hotkey::apply(&handle, &settings.hotkey) {
                eprintln!("could not register shortcut {}: {e}", settings.hotkey);
            }
            if !cfg!(debug_assertions) {
                commands::sync_autostart(&handle, settings.launch_at_startup);
            }
            clipboard::spawn_watcher(handle.clone());
            if settings.check_updates {
                tray::check_updates_in_background(handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                WindowEvent::Focused(false) => popup::on_blur(window.app_handle()),
                WindowEvent::Moved(pos) => popup::on_moved(window.app_handle(), *pos),
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_entries,
            commands::get_image,
            commands::paste_entry,
            commands::delete_entry,
            commands::toggle_pin,
            commands::set_entry_tags,
            commands::clear_history,
            commands::hide_popup,
            commands::open_settings,
            commands::get_settings,
            commands::save_settings,
            commands::app_info,
            commands::export_settings,
            commands::import_settings,
            commands::check_updates,
            commands::pending_update,
            commands::install_update,
            commands::reset_popup_position,
            commands::set_pinned_collapsed,
            commands::native_win_v_disabled,
            commands::set_native_win_v_disabled,
            commands::open_url,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Simple Clipboard")
        .run(|_app, event| {
            // Keep running in the tray when the settings window is closed.
            if let RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
