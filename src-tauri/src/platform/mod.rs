#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(not(windows))]
mod unix {
    use std::{process::Command, thread, time::Duration};

    pub fn remember_foreground() {}

    /// Sends Ctrl+V to the focused window via xdotool (X11) or wtype (Wayland).
    pub fn send_paste() {
        thread::sleep(Duration::from_millis(150));
        let tried = [
            ("xdotool", &["key", "--clearmodifiers", "ctrl+v"][..]),
            ("wtype", &["-M", "ctrl", "v", "-m", "ctrl"][..]),
        ];
        for (cmd, args) in tried {
            if Command::new(cmd).args(args).status().map(|s| s.success()).unwrap_or(false) {
                return;
            }
        }
    }

    pub fn clipboard_excluded() -> bool {
        false
    }

    pub fn native_win_v_disabled() -> bool {
        false
    }

    pub fn set_native_win_v_disabled(_disabled: bool) -> Result<(), String> {
        Err("Only available on Windows".into())
    }
}
#[cfg(not(windows))]
pub use self::unix::*;
