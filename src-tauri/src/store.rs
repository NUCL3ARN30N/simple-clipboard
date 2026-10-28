//! Persistent state: settings, clipboard history and image files on disk.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tauri::{AppHandle, Manager};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Text,
    Image,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    pub kind: Kind,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    pub hash: u64,
    pub created: u64,
    #[serde(default)]
    pub pinned: bool,
    /// Tag ids, only meaningful for pinned entries.
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub max_items: usize,
    pub images_enabled: bool,
    pub pinned_enabled: bool,
    /// "left" or "right"
    pub pinned_side: String,
    /// Pinned palette folded to a narrow rail; toggled from the popup and remembered.
    pub pinned_collapsed: bool,
    /// Global shortcut. "Super+V" on Windows replaces the native clipboard history.
    pub hotkey: String,
    pub auto_paste: bool,
    pub launch_at_startup: bool,
    pub check_updates: bool,
    /// "system", "light" or "dark"
    pub theme: String,
    pub accent: String,
    pub tags: Vec<Tag>,
    /// Open the popup where it was last dragged to instead of near the mouse.
    pub remember_position: bool,
    /// Physical screen position of the dragged popup, managed by the backend only.
    pub popup_position: Option<[i32; 2]>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_items: 50,
            images_enabled: true,
            pinned_enabled: true,
            pinned_side: "right".into(),
            pinned_collapsed: false,
            hotkey: "Super+V".into(),
            auto_paste: true,
            launch_at_startup: true,
            check_updates: true,
            theme: "system".into(),
            accent: "#6750A4".into(),
            tags: vec![
                Tag { id: "work".into(), name: "Work".into(), color: "#1E88E5".into() },
                Tag { id: "personal".into(), name: "Personal".into(), color: "#43A047".into() },
                Tag { id: "code".into(), name: "Code".into(), color: "#F4511E".into() },
            ],
            remember_position: true,
            popup_position: None,
        }
    }
}

impl Settings {
    pub fn sanitized(mut self) -> Self {
        self.max_items = self.max_items.clamp(1, 1000);
        if self.pinned_side != "left" {
            self.pinned_side = "right".into();
        }
        if !["system", "light", "dark"].contains(&self.theme.as_str()) {
            self.theme = "system".into();
        }
        self.hotkey = self.hotkey.trim().to_string();
        self.tags.retain(|t| !t.id.is_empty());
        self
    }
}

#[derive(Default, Serialize, Deserialize)]
struct HistoryFile {
    next_id: u64,
    entries: Vec<Entry>,
}

pub struct Data {
    pub settings: Settings,
    /// Newest first.
    pub entries: Vec<Entry>,
    next_id: u64,
}

impl Data {
    pub fn alloc_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Moves an existing entry with this hash to the top. Returns false if none exists.
    pub fn bump(&mut self, hash: u64) -> bool {
        let Some(pos) = self.entries.iter().position(|e| e.hash == hash) else {
            return false;
        };
        let mut entry = self.entries.remove(pos);
        entry.created = now_ms();
        self.entries.insert(0, entry);
        true
    }

    pub fn insert(&mut self, entry: Entry) {
        self.entries.insert(0, entry);
    }

    /// Drops the oldest unpinned entries beyond `max_items`. Returns the removed entries.
    pub fn trim(&mut self) -> Vec<Entry> {
        let max = self.settings.max_items;
        let mut kept = 0;
        let mut removed = Vec::new();
        self.entries.retain(|e| {
            if e.pinned {
                return true;
            }
            kept += 1;
            if kept > max {
                removed.push(e.clone());
                false
            } else {
                true
            }
        });
        removed
    }

    pub fn remove(&mut self, id: u64) -> Option<Entry> {
        let pos = self.entries.iter().position(|e| e.id == id)?;
        Some(self.entries.remove(pos))
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }
}

pub struct AppState {
    pub dir: PathBuf,
    pub data: Mutex<Data>,
    /// Long-lived clipboard handle used for writing (on X11 the owner must stay alive).
    pub writer: Mutex<Option<arboard::Clipboard>>,
}

impl AppState {
    pub fn load(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(dir.join("images"));
        let settings: Settings = read_json(&dir.join("settings.json")).unwrap_or_default();
        let history: HistoryFile = read_json(&dir.join("history.json")).unwrap_or_default();
        let next_id = history.entries.iter().map(|e| e.id).max().unwrap_or(0).max(history.next_id);
        Self {
            dir,
            data: Mutex::new(Data { settings: settings.sanitized(), entries: history.entries, next_id }),
            writer: Mutex::new(None),
        }
    }

    pub fn image_path(&self, id: u64, thumb: bool) -> PathBuf {
        let name = if thumb { format!("{id}_thumb.png") } else { format!("{id}.png") };
        self.dir.join("images").join(name)
    }

    pub fn save_settings(&self, data: &Data) {
        write_json(&self.dir.join("settings.json"), &data.settings);
    }

    pub fn save_history(&self, data: &Data) {
        let file = HistoryFile { next_id: data.next_id, entries: data.entries.clone() };
        write_json(&self.dir.join("history.json"), &file);
    }

    pub fn delete_files(&self, removed: &[Entry]) {
        for e in removed.iter().filter(|e| e.kind == Kind::Image) {
            let _ = fs::remove_file(self.image_path(e.id, false));
            let _ = fs::remove_file(self.image_path(e.id, true));
        }
    }
}

/// Uses `<exe dir>/data` when a `portable` marker file sits next to the executable.
pub fn data_dir(app: &AppHandle) -> PathBuf {
    if let Some(dir) = portable_dir() {
        return dir.join("data");
    }
    app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("data"))
}

/// The executable's folder when it contains a `portable` marker file.
pub fn portable_dir() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    (exe_dir.join("portable").exists() || exe_dir.join(".portable").exists()).then_some(exe_dir)
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) {
    let Ok(raw) = serde_json::to_string(value) else { return };
    let tmp = path.with_extension("json.tmp");
    if fs::write(&tmp, raw).is_ok() {
        let _ = fs::rename(&tmp, path);
    }
}
