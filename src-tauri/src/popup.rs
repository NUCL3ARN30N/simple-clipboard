//! Showing, positioning and hiding the clipboard popup window.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindow};

use crate::{
    platform,
    store::{AppState, Settings},
};

const WIDTH: f64 = 380.0;
const PINNED_WIDTH: f64 = 300.0;
/// Width of the pinned palette when collapsed to a rail.
const RAIL_WIDTH: f64 = 48.0;
const HEIGHT: f64 = 540.0;
const MARGIN: f64 = 12.0;

static SHOWN_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Set when the user dragged the popup; the position is persisted on hide.
static POSITION_DIRTY: AtomicBool = AtomicBool::new(false);
static WATCHING_FOCUS: AtomicBool = AtomicBool::new(false);
static LAST_MOVED: Mutex<Option<Instant>> = Mutex::new(None);

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("main")
}

pub fn toggle(app: &AppHandle) {
    match window(app) {
        Some(w) if w.is_visible().unwrap_or(false) => hide(app),
        _ => show(app),
    }
}

pub fn show(app: &AppHandle) {
    let Some(w) = window(app) else { return };
    if !w.is_visible().unwrap_or(false) {
        platform::remember_foreground();
    }
    let (width, saved) = {
        let state = app.state::<AppState>();
        let data = state.data.lock().unwrap();
        let s = &data.settings;
        (popup_width(s), s.popup_position.filter(|_| s.remember_position))
    };

    *SHOWN_AT.lock().unwrap() = Some(Instant::now());
    let _ = w.set_size(LogicalSize::new(width, HEIGHT));
    match saved.filter(|&[x, y]| on_screen(app, x, y)) {
        Some([x, y]) => {
            let _ = w.set_position(PhysicalPosition::new(x, y));
        }
        None => place_near_cursor(app, &w, width, HEIGHT),
    }
    let _ = w.show();
    let _ = w.unminimize();
    let _ = w.set_focus();
    let _ = w.emit("popup-shown", ());
}

fn popup_width(s: &Settings) -> f64 {
    match (s.pinned_enabled, s.pinned_collapsed) {
        (false, _) => WIDTH,
        (true, true) => WIDTH + RAIL_WIDTH,
        (true, false) => WIDTH + PINNED_WIDTH,
    }
}

/// Collapses or expands the pinned palette, resizing the open popup so the
/// history column stays where it is.
pub fn set_pinned_collapsed(app: &AppHandle, collapsed: bool) {
    let settings = {
        let state = app.state::<AppState>();
        let mut data = state.data.lock().unwrap();
        data.settings.pinned_collapsed = collapsed;
        state.save_settings(&data);
        data.settings.clone()
    };
    if let Some(w) = window(app) {
        let width = popup_width(&settings);
        let scale = w.scale_factor().unwrap_or(1.0);
        if let (Ok(pos), Ok(size)) = (w.outer_position(), w.outer_size()) {
            let _ = w.set_size(LogicalSize::new(width, HEIGHT));
            if settings.pinned_side == "left" {
                let delta = size.width as i32 - (width * scale).round() as i32;
                let _ = w.set_position(PhysicalPosition::new(pos.x + delta, pos.y));
            }
        }
    }
    let _ = app.emit("settings-changed", &settings);
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = window(app) {
        let _ = w.hide();
    }
    if POSITION_DIRTY.swap(false, Ordering::SeqCst) {
        let state = app.state::<AppState>();
        let data = state.data.lock().unwrap();
        state.save_settings(&data);
    }
}

fn shown_within(ms: u128) -> bool {
    SHOWN_AT.lock().unwrap().map(|t| t.elapsed().as_millis() < ms).unwrap_or(false)
}

/// Hides the popup when it loses focus, ignoring the focus churn right after showing.
pub fn on_blur(app: &AppHandle) {
    if shown_within(300) {
        return;
    }
    if !popup_is_foreground(app) {
        hide(app);
        return;
    }
    // Focus only moved from the webview to our own window frame, which happens when
    // dragging. Keep the popup open until another window really becomes active.
    if WATCHING_FOCUS.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let blurred_at = Instant::now();
    thread::spawn(move || {
        let mut refocused = false;
        loop {
            thread::sleep(Duration::from_millis(150));
            let Some(w) = window(&app).filter(|w| w.is_visible().unwrap_or(false)) else { break };
            if !popup_is_foreground(&app) {
                hide(&app);
                break;
            }
            // Give keyboard focus back to the page once the drag has settled.
            let last_activity = LAST_MOVED.lock().unwrap().map_or(blurred_at, |t| t.max(blurred_at));
            if !refocused && last_activity.elapsed() > Duration::from_millis(300) {
                refocused = true;
                let _ = w.set_focus();
            }
        }
        WATCHING_FOCUS.store(false, Ordering::SeqCst);
    });
}

#[cfg(windows)]
fn popup_is_foreground(app: &AppHandle) -> bool {
    window(app)
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize == platform::foreground_window())
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn popup_is_foreground(_app: &AppHandle) -> bool {
    false
}

/// Remembers where the user dragged the popup (moves caused by `show` are ignored).
pub fn on_moved(app: &AppHandle, pos: PhysicalPosition<i32>) {
    let visible = window(app).and_then(|w| w.is_visible().ok()).unwrap_or(false);
    if !visible || shown_within(500) {
        return;
    }
    *LAST_MOVED.lock().unwrap() = Some(Instant::now());
    let state = app.state::<AppState>();
    let mut data = state.data.lock().unwrap();
    if data.settings.remember_position {
        data.settings.popup_position = Some([pos.x, pos.y]);
        POSITION_DIRTY.store(true, Ordering::SeqCst);
    }
}

fn on_screen(app: &AppHandle, x: i32, y: i32) -> bool {
    app.monitor_from_point(x as f64 + 40.0, y as f64 + 20.0).ok().flatten().is_some()
}

fn place_near_cursor(app: &AppHandle, w: &WebviewWindow, width: f64, height: f64) {
    let Ok(cursor) = app.cursor_position() else {
        let _ = w.center();
        return;
    };
    let monitor = app
        .monitor_from_point(cursor.x, cursor.y)
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        let _ = w.center();
        return;
    };

    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let (pw, ph, margin) = (width * scale, height * scale, MARGIN * scale);
    let (left, top) = (area.position.x as f64, area.position.y as f64);
    let (right, bottom) = (left + area.size.width as f64, top + area.size.height as f64);

    let mut x = cursor.x - pw / 2.0;
    let mut y = cursor.y + margin;
    if y + ph > bottom - margin {
        y = cursor.y - ph - margin;
    }
    x = x.min(right - pw - margin).max(left + margin);
    y = y.min(bottom - ph - margin).max(top + margin);
    let _ = w.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}
