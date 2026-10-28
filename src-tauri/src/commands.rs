//! Tauri commands invoked from the popup and settings windows.

use std::fs;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::{
    clipboard, hotkey, platform, popup,
    store::{now_ms, AppState, Entry, Kind, Settings},
    updates::{self, UpdateInfo},
};

const PREVIEW_CHARS: usize = 600;

#[derive(Serialize)]
pub struct EntryView {
    id: u64,
    kind: Kind,
    preview: String,
    chars: usize,
    width: u32,
    height: u32,
    created: u64,
    pinned: bool,
    tags: Vec<String>,
}

impl From<&Entry> for EntryView {
    fn from(e: &Entry) -> Self {
        Self {
            id: e.id,
            kind: e.kind,
            preview: e.text.chars().take(PREVIEW_CHARS).collect(),
            chars: e.text.chars().count(),
            width: e.width,
            height: e.height,
            created: e.created,
            pinned: e.pinned,
            tags: e.tags.clone(),
        }
    }
}

fn changed(app: &AppHandle) {
    let _ = app.emit("history-changed", ());
}

#[tauri::command]
pub fn get_entries(state: State<'_, AppState>, query: String) -> Vec<EntryView> {
    let data = state.data.lock().unwrap();
    let q = query.trim().to_lowercase();
    let tag_hits: Vec<&str> = data
        .settings
        .tags
        .iter()
        .filter(|t| !q.is_empty() && t.name.to_lowercase().contains(&q))
        .map(|t| t.id.as_str())
        .collect();
    data.entries
        .iter()
        .filter(|e| {
            q.is_empty()
                || (e.kind == Kind::Text && e.text.to_lowercase().contains(&q))
                || (e.kind == Kind::Image && "image".contains(&q))
                || e.tags.iter().any(|t| tag_hits.contains(&t.as_str()))
        })
        .map(EntryView::from)
        .collect()
}

#[tauri::command]
pub async fn get_image(state: State<'_, AppState>, id: u64, thumb: bool) -> Result<String, String> {
    let mut path = state.image_path(id, thumb);
    if !path.exists() {
        path = state.image_path(id, false);
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

#[tauri::command]
pub async fn paste_entry(app: AppHandle, state: State<'_, AppState>, id: u64) -> Result<(), String> {
    let (entry, auto_paste) = {
        let data = state.data.lock().unwrap();
        let entry = data.entries.iter().find(|e| e.id == id).cloned().ok_or("Entry not found")?;
        (entry, data.settings.auto_paste)
    };
    clipboard::write_entry(&state, &entry)?;
    popup::hide(&app);
    if auto_paste {
        platform::send_paste();
    }
    Ok(())
}

#[tauri::command]
pub fn delete_entry(app: AppHandle, state: State<'_, AppState>, id: u64) {
    let removed = {
        let mut data = state.data.lock().unwrap();
        let removed = data.remove(id);
        state.save_history(&data);
        removed
    };
    if let Some(e) = removed {
        state.delete_files(&[e]);
    }
    changed(&app);
}

#[tauri::command]
pub fn toggle_pin(app: AppHandle, state: State<'_, AppState>, id: u64) {
    let removed = {
        let mut data = state.data.lock().unwrap();
        if let Some(e) = data.get_mut(id) {
            e.pinned = !e.pinned;
            if !e.pinned {
                e.tags.clear();
            }
        }
        let removed = data.trim();
        state.save_history(&data);
        removed
    };
    state.delete_files(&removed);
    changed(&app);
}

#[tauri::command]
pub fn set_entry_tags(app: AppHandle, state: State<'_, AppState>, id: u64, tags: Vec<String>) {
    {
        let mut data = state.data.lock().unwrap();
        if let Some(e) = data.get_mut(id) {
            e.tags = tags;
        }
        state.save_history(&data);
    }
    changed(&app);
}

#[tauri::command]
pub fn clear_history(app: AppHandle, state: State<'_, AppState>) {
    let removed: Vec<Entry> = {
        let mut data = state.data.lock().unwrap();
        let (removed, kept) = data.entries.drain(..).partition(|e| !e.pinned);
        data.entries = kept;
        state.save_history(&data);
        removed
    };
    state.delete_files(&removed);
    changed(&app);
}

#[tauri::command]
pub fn hide_popup(app: AppHandle) {
    popup::hide(&app);
}

#[tauri::command]
pub fn open_settings(app: AppHandle, section: Option<String>) {
    open_settings_window(&app, section.as_deref());
}

pub fn open_settings_window(app: &AppHandle, section: Option<&str>) {
    popup::hide(app);
    if let Some(w) = app.get_webview_window("settings") {
        if let Some(section) = section {
            let _ = w.emit("settings-section", section);
        }
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let url = match section {
        Some(s) => format!("settings.html#{s}"),
        None => "settings.html".into(),
    };
    // Building a webview window on the main thread (sync commands, menu handlers)
    // deadlocks on Windows, so always build from a worker thread.
    let app = app.clone();
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App(url.into()))
            .title("Simple Clipboard – Settings")
            .inner_size(600.0, 720.0)
            .min_inner_size(460.0, 480.0)
            .center()
            .build();
        if let Err(e) = built {
            eprintln!("failed to open settings: {e}");
        }
    });
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.data.lock().unwrap().settings.clone()
}

#[tauri::command]
pub fn app_info() -> serde_json::Value {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "repo": updates::REPO_URL,
    })
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> Result<Settings, String> {
    apply_settings(&app, &state, settings)
}

/// Stores new settings and applies side effects (shortcut, autostart, trimming).
fn apply_settings(app: &AppHandle, state: &AppState, settings: Settings) -> Result<Settings, String> {
    let mut settings = settings.sanitized();
    let old = state.data.lock().unwrap().settings.clone();
    // The UI may hold a stale copy; the dragged position is owned by the backend.
    settings.popup_position = if settings.remember_position { old.popup_position } else { None };

    let mut error = None;
    if settings.hotkey != old.hotkey {
        if let Err(e) = hotkey::apply(app, &settings.hotkey) {
            error = Some(format!("Shortcut \"{}\" could not be registered: {e}", settings.hotkey));
            settings.hotkey = old.hotkey.clone();
            let _ = hotkey::apply(app, &old.hotkey);
        }
    }
    if settings.launch_at_startup != old.launch_at_startup {
        sync_autostart(app, settings.launch_at_startup);
    }

    let removed = {
        let mut data = state.data.lock().unwrap();
        // Tags that no longer exist are removed from entries.
        let valid: Vec<String> = settings.tags.iter().map(|t| t.id.clone()).collect();
        for e in data.entries.iter_mut() {
            e.tags.retain(|t| valid.contains(t));
        }
        data.settings = settings.clone();
        let removed = data.trim();
        state.save_settings(&data);
        state.save_history(&data);
        removed
    };
    state.delete_files(&removed);
    let _ = app.emit("settings-changed", &settings);
    changed(app);

    match error {
        Some(e) => Err(e),
        None => Ok(settings),
    }
}

pub fn sync_autostart(app: &AppHandle, enabled: bool) {
    let autolaunch = app.autolaunch();
    if autolaunch.is_enabled().unwrap_or(false) != enabled {
        let result = if enabled { autolaunch.enable() } else { autolaunch.disable() };
        if let Err(e) = result {
            eprintln!("autostart: {e}");
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Backup {
    app: String,
    version: String,
    settings: Settings,
    #[serde(default)]
    pinned: Vec<PinnedBackup>,
}

#[derive(Serialize, Deserialize)]
struct PinnedBackup {
    text: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[tauri::command]
pub async fn export_settings(app: AppHandle, state: State<'_, AppState>) -> Result<Option<String>, String> {
    let backup = {
        let data = state.data.lock().unwrap();
        Backup {
            app: "simple-clipboard".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            settings: data.settings.clone(),
            pinned: data
                .entries
                .iter()
                .filter(|e| e.pinned && e.kind == Kind::Text)
                .map(|e| PinnedBackup { text: e.text.clone(), tags: e.tags.clone() })
                .collect(),
        }
    };
    let Some(path) = app
        .dialog()
        .file()
        .set_title("Export settings")
        .set_file_name("simple-clipboard-settings.json")
        .add_filter("JSON", &["json"])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| e.to_string())?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
pub async fn import_settings(app: AppHandle, state: State<'_, AppState>) -> Result<Option<Settings>, String> {
    let Some(path) = app
        .dialog()
        .file()
        .set_title("Import settings")
        .add_filter("JSON", &["json"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let backup: Backup = match serde_json::from_str(&raw) {
        Ok(b) => b,
        Err(_) => {
            let settings: Settings = serde_json::from_str(&raw).map_err(|e| format!("Not a valid settings file: {e}"))?;
            Backup { app: String::new(), version: String::new(), settings, pinned: vec![] }
        }
    };

    {
        let mut data = state.data.lock().unwrap();
        for p in backup.pinned {
            let hash = clipboard::hash_text(&p.text);
            if let Some(e) = data.entries.iter_mut().find(|e| e.hash == hash) {
                e.pinned = true;
                e.tags = p.tags;
                continue;
            }
            let id = data.alloc_id();
            data.entries.push(Entry {
                id,
                kind: Kind::Text,
                text: p.text,
                width: 0,
                height: 0,
                hash,
                created: now_ms(),
                pinned: true,
                tags: p.tags,
            });
        }
    }
    apply_settings(&app, &state, backup.settings).map(Some)
}

#[tauri::command]
pub async fn check_updates() -> Result<UpdateInfo, String> {
    tauri::async_runtime::spawn_blocking(updates::check)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn reset_popup_position(app: AppHandle, state: State<'_, AppState>) {
    let settings = {
        let mut data = state.data.lock().unwrap();
        data.settings.popup_position = None;
        state.save_settings(&data);
        data.settings.clone()
    };
    let _ = app.emit("settings-changed", &settings);
}

#[tauri::command]
pub fn native_win_v_disabled() -> bool {
    platform::native_win_v_disabled()
}

/// Turns the built-in Windows clipboard shortcut off (or restores it). Restarts Explorer.
#[tauri::command]
pub async fn set_native_win_v_disabled(disabled: bool) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || platform::set_native_win_v_disabled(disabled))
        .await
        .map_err(|e| e.to_string())??;
    Ok(platform::native_win_v_disabled())
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("Only https links can be opened".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}
