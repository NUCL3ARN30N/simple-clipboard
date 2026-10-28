//! Global shortcut registration. On Windows, "Super+V" is handled by a keyboard
//! hook because the OS reserves it; everything else goes through the plugin.

use tauri::AppHandle;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

#[cfg_attr(not(windows), allow(dead_code))]
pub fn is_win_v(hotkey: &str) -> bool {
    let norm = hotkey.replace(' ', "").to_lowercase();
    norm == "super+v" || norm == "win+v" || norm == "meta+v"
}

pub fn apply(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();

    #[cfg(windows)]
    {
        let hook = is_win_v(hotkey);
        crate::platform::set_win_v_enabled(hook);
        if hook {
            return Ok(());
        }
    }

    if hotkey.is_empty() {
        return Ok(());
    }
    let normalized = hotkey.replace("Win+", "Super+").replace("Meta+", "Super+");
    shortcuts.register(normalized.as_str()).map_err(|e| e.to_string())
}
