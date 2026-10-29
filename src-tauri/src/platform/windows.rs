//! Windows specifics: a low-level keyboard hook that takes over Win+V from the
//! built-in clipboard history, focus restoration and Ctrl+V injection.

use std::{
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicBool, AtomicIsize, Ordering},
        OnceLock,
    },
    thread,
    time::Duration,
};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::{
        DataExchange::{GetClipboardSequenceNumber, IsClipboardFormatAvailable, RegisterClipboardFormatW},
        LibraryLoader::GetModuleHandleW,
    },
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
            VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
        },
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId,
            SetForegroundWindow, SetWindowsHookExW, TranslateMessage, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
            MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
        },
    },
};

const VK_V: u32 = 0x56;
/// Unassigned virtual key, injected so releasing Win does not open the Start menu.
const VK_MASK: u16 = 0xE8;

static HOOK_ENABLED: AtomicBool = AtomicBool::new(false);
static SWALLOWING_V: AtomicBool = AtomicBool::new(false);
static TRIGGER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();
static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

/// Installs the keyboard hook on its own message-loop thread. `trigger` must return quickly.
pub fn install_hook(trigger: impl Fn() + Send + Sync + 'static) {
    if TRIGGER.set(Box::new(trigger)).is_err() {
        return;
    }
    thread::spawn(|| unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), GetModuleHandleW(null()), 0);
        if hook.is_null() {
            eprintln!("failed to install keyboard hook");
            return;
        }
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

pub fn set_win_v_enabled(enabled: bool) {
    HOOK_ENABLED.store(enabled, Ordering::SeqCst);
}

fn key_down(vk: u16) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && HOOK_ENABLED.load(Ordering::Relaxed) {
        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
        if kb.vkCode == VK_V && kb.flags & LLKHF_INJECTED == 0 {
            let msg = wparam as u32;
            let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            if down {
                if SWALLOWING_V.load(Ordering::Relaxed) {
                    // Auto-repeat while held.
                    return 1;
                }
                let win = key_down(VK_LWIN) || key_down(VK_RWIN);
                if win && !key_down(VK_CONTROL) && !key_down(VK_MENU) && !key_down(VK_SHIFT) {
                    SWALLOWING_V.store(true, Ordering::Relaxed);
                    send_keys(&[(VK_MASK, false), (VK_MASK, true)]);
                    if let Some(trigger) = TRIGGER.get() {
                        trigger();
                    }
                    return 1;
                }
            } else if SWALLOWING_V.swap(false, Ordering::Relaxed) {
                return 1;
            }
        }
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

fn send_keys(keys: &[(u16, bool)]) {
    let inputs: Vec<INPUT> = keys
        .iter()
        .map(|&(vk, up)| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        })
        .collect();
    unsafe {
        SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

/// Remembers the window that had focus before the popup opened.
pub fn remember_foreground() {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if !hwnd.is_null() && pid != std::process::id() {
            PREV_FOREGROUND.store(hwnd as isize, Ordering::SeqCst);
        }
    }
}

pub fn foreground_window() -> isize {
    unsafe { GetForegroundWindow() as isize }
}

/// Re-focuses the previous window and sends Ctrl+V to it.
pub fn send_paste() {
    thread::sleep(Duration::from_millis(40));
    let prev = PREV_FOREGROUND.load(Ordering::SeqCst);
    if prev != 0 {
        unsafe {
            SetForegroundWindow(prev as HWND);
        }
    }
    thread::sleep(Duration::from_millis(60));
    let v = VK_V as u16;
    send_keys(&[(VK_CONTROL, false), (v, false), (v, true), (VK_CONTROL, true)]);
}

pub fn clipboard_sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// Password managers mark sensitive clipboard content with these formats.
pub fn clipboard_excluded() -> bool {
    static FORMATS: OnceLock<Vec<u32>> = OnceLock::new();
    let formats = FORMATS.get_or_init(|| {
        ["ExcludeClipboardContentFromMonitorProcessing", "Clipboard Viewer Ignore"]
            .iter()
            .map(|name| {
                let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
                unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
            })
            .filter(|&f| f != 0)
            .collect()
    });
    formats.iter().any(|&f| unsafe { IsClipboardFormatAvailable(f) != 0 })
}

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Rewrites our autostart entry to the quoted path of the running exe. The autostart plugin
/// writes the path unquoted, which Windows cannot launch when it contains spaces
/// (`C:\Program Files\...`), and never updates it after the exe moved.
pub fn fix_autostart_entry(name: &str) {
    let Ok(exe) = std::env::current_exe() else { return };
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let Ok(key) = hkcu.open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_READ | winreg::enums::KEY_WRITE) else {
        return;
    };
    let Ok(current) = key.get_value::<String, _>(name) else { return };
    let wanted = format!("\"{}\"", exe.display());
    if current != wanted {
        if let Err(e) = key.set_value(name, &wanted) {
            eprintln!("autostart: {e}");
        }
    }
}

const ADVANCED_KEY: &str =r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const DISABLED_HOTKEYS: &str = "DisabledHotkeys";

/// True when Explorer's own Win+V (clipboard history) is switched off via `DisabledHotkeys`.
pub fn native_win_v_disabled() -> bool {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    hkcu.open_subkey(ADVANCED_KEY)
        .and_then(|k| k.get_value::<String, _>(DISABLED_HOTKEYS))
        .map(|v| v.to_ascii_uppercase().contains('V'))
        .unwrap_or(false)
}

/// Adds or removes "V" from Explorer's `DisabledHotkeys` and restarts Explorer to apply it.
pub fn set_native_win_v_disabled(disabled: bool) -> Result<(), String> {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(ADVANCED_KEY).map_err(|e| e.to_string())?;
    let current: String = key.get_value(DISABLED_HOTKEYS).unwrap_or_default();
    let mut keys: String = current.chars().filter(|c| !c.eq_ignore_ascii_case(&'v')).collect();
    if disabled {
        keys.push('V');
    }
    if keys.is_empty() {
        match key.delete_value(DISABLED_HOTKEYS) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => {}
        }
    } else {
        key.set_value(DISABLED_HOTKEYS, &keys).map_err(|e| e.to_string())?;
    }
    restart_explorer();
    Ok(())
}

fn restart_explorer() {
    use std::{os::windows::process::CommandExt, process::Command};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let run = |cmd: &str, args: &[&str]| {
        Command::new(cmd).args(args).creation_flags(CREATE_NO_WINDOW).output()
    };
    let _ = run("taskkill", &["/f", "/im", "explorer.exe"]);
    // Windows usually restarts the shell by itself; start it only if it didn't.
    for _ in 0..10 {
        thread::sleep(Duration::from_millis(300));
        let running = run("tasklist", &["/fi", "imagename eq explorer.exe", "/nh"])
            .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("explorer.exe"))
            .unwrap_or(false);
        if running {
            return;
        }
    }
    let _ = Command::new("explorer.exe").spawn();
}
