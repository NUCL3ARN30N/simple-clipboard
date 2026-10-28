//! Clipboard watching (history capture) and writing entries back to the clipboard.

use std::{
    borrow::Cow,
    collections::hash_map::DefaultHasher,
    hash::Hasher,
    thread,
    time::Duration,
};

use arboard::{Clipboard, ImageData};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    platform,
    store::{now_ms, AppState, Entry, Kind},
};

#[cfg(windows)]
const POLL: Duration = Duration::from_millis(250);
#[cfg(not(windows))]
const POLL: Duration = Duration::from_millis(700);

/// Images above this pixel count are ignored to keep memory and disk usage sane.
const MAX_IMAGE_PIXELS: usize = 40_000_000;
const THUMB_SIZE: u32 = 320;

pub fn hash_text(text: &str) -> u64 {
    hash_bytes(b'T', text.as_bytes())
}

fn hash_bytes(prefix: u8, bytes: &[u8]) -> u64 {
    let mut h = DefaultHasher::new();
    h.write_u8(prefix);
    h.write(bytes);
    h.finish()
}

pub fn spawn_watcher(app: AppHandle) {
    thread::spawn(move || {
        let mut cb = loop {
            match Clipboard::new() {
                Ok(cb) => break cb,
                Err(_) => thread::sleep(Duration::from_secs(1)),
            }
        };
        let mut last: Option<u64> = None;
        #[cfg(windows)]
        let mut last_seq = 0u32;

        loop {
            thread::sleep(POLL);
            #[cfg(windows)]
            {
                let seq = platform::clipboard_sequence();
                if seq == last_seq {
                    continue;
                }
                last_seq = seq;
            }
            if platform::clipboard_excluded() {
                continue;
            }

            if let Ok(text) = cb.get_text() {
                if text.trim().is_empty() {
                    continue;
                }
                let hash = hash_text(&text);
                if last != Some(hash) {
                    last = Some(hash);
                    add_text(&app, text, hash);
                }
                continue;
            }

            let images = app.state::<AppState>().data.lock().unwrap().settings.images_enabled;
            if !images {
                continue;
            }
            if let Ok(img) = cb.get_image() {
                if img.width * img.height > MAX_IMAGE_PIXELS {
                    continue;
                }
                let hash = hash_bytes(b'I', &img.bytes);
                if last != Some(hash) {
                    last = Some(hash);
                    add_image(&app, img, hash);
                }
            }
        }
    });
}

fn add_text(app: &AppHandle, text: String, hash: u64) {
    let state = app.state::<AppState>();
    let removed = {
        let mut data = state.data.lock().unwrap();
        if !data.bump(hash) {
            let id = data.alloc_id();
            data.insert(Entry {
                id,
                kind: Kind::Text,
                text,
                width: 0,
                height: 0,
                hash,
                created: now_ms(),
                pinned: false,
                tags: vec![],
            });
        }
        let removed = data.trim();
        state.save_history(&data);
        removed
    };
    state.delete_files(&removed);
    let _ = app.emit("history-changed", ());
}

fn add_image(app: &AppHandle, img: ImageData, hash: u64) {
    let state = app.state::<AppState>();
    let id = {
        let mut data = state.data.lock().unwrap();
        if data.bump(hash) {
            state.save_history(&data);
            drop(data);
            let _ = app.emit("history-changed", ());
            return;
        }
        data.alloc_id()
    };

    // Encode outside the lock; PNG encoding of large screenshots takes a moment.
    let (w, h) = (img.width as u32, img.height as u32);
    let Some(rgba) = image::RgbaImage::from_raw(w, h, img.bytes.into_owned()) else { return };
    if rgba.save(state.image_path(id, false)).is_err() {
        return;
    }
    let (tw, th) = if w > THUMB_SIZE || h > THUMB_SIZE {
        let scale = THUMB_SIZE as f32 / w.max(h) as f32;
        (((w as f32 * scale) as u32).max(1), ((h as f32 * scale) as u32).max(1))
    } else {
        (w, h)
    };
    let _ = image::imageops::thumbnail(&rgba, tw, th).save(state.image_path(id, true));

    let removed = {
        let mut data = state.data.lock().unwrap();
        data.insert(Entry {
            id,
            kind: Kind::Image,
            text: String::new(),
            width: w,
            height: h,
            hash,
            created: now_ms(),
            pinned: false,
            tags: vec![],
        });
        let removed = data.trim();
        state.save_history(&data);
        removed
    };
    state.delete_files(&removed);
    let _ = app.emit("history-changed", ());
}

pub fn write_entry(state: &AppState, entry: &Entry) -> Result<(), String> {
    let mut guard = state.writer.lock().unwrap();
    if guard.is_none() {
        *guard = Some(Clipboard::new().map_err(|e| e.to_string())?);
    }
    let cb = guard.as_mut().unwrap();
    match entry.kind {
        Kind::Text => cb.set_text(entry.text.clone()).map_err(|e| e.to_string()),
        Kind::Image => {
            let img = image::open(state.image_path(entry.id, false))
                .map_err(|e| e.to_string())?
                .to_rgba8();
            cb.set_image(ImageData {
                width: img.width() as usize,
                height: img.height() as usize,
                bytes: Cow::Owned(img.into_raw()),
            })
            .map_err(|e| e.to_string())
        }
    }
}
