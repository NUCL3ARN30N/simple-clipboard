//! Checks GitHub releases for a newer version, downloads the matching asset and installs it.

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    time::{Duration, Instant},
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

use crate::store;


pub const REPO_URL: &str = "https://github.com/NUCL3ARN30N/simple-clipboard";
const API_URL: &str = "https://api.github.com/repos/NUCL3ARN30N/simple-clipboard/releases/latest";
const USER_AGENT: &str = concat!("simple-clipboard/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Serialize)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    /// "sha256:<hex>" as reported by GitHub, if available.
    pub digest: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub available: bool,
    pub url: String,
    pub notes: String,
    /// How this copy was installed: "installer", "msi", "portable", "deb" or "manual".
    pub install_kind: &'static str,
    /// Release file matching `install_kind`; None means the user has to update manually.
    pub asset: Option<Asset>,
}

/// Latest known update, shared with the tray, popup and settings window.
#[derive(Default)]
pub struct PendingUpdate(pub Mutex<Option<UpdateInfo>>);

#[derive(Clone, Copy, PartialEq, Eq)]
enum InstallKind {
    Installer,
    Msi,
    Portable,
    Deb,
    Manual,
}

impl InstallKind {
    fn detect() -> Self {
        if cfg!(windows) {
            if store::portable_dir().is_some() {
                return Self::Portable;
            }
            let exe = std::env::current_exe().unwrap_or_default().to_string_lossy().to_lowercase();
            // NSIS installs per user into AppData; the MSI goes to Program Files.
            if exe.contains("\\program files") {
                Self::Msi
            } else {
                Self::Installer
            }
        } else if cfg!(target_os = "linux") && std::env::current_exe().map(|p| p.starts_with("/usr")).unwrap_or(false) {
            Self::Deb
        } else {
            Self::Manual
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Installer => "installer",
            Self::Msi => "msi",
            Self::Portable => "portable",
            Self::Deb => "deb",
            Self::Manual => "manual",
        }
    }

    /// Suffix of the release asset built by the GitHub workflow for this kind.
    fn asset_suffix(self) -> Option<&'static str> {
        match self {
            Self::Installer => Some("-setup.exe"),
            Self::Msi => Some(".msi"),
            Self::Portable => Some("_portable.zip"),
            Self::Deb => Some("_amd64.deb"),
            Self::Manual => None,
        }
    }
}

pub fn check() -> Result<UpdateInfo, String> {
    let response = ureq::get(API_URL)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(10))
        .call();
    let json: serde_json::Value = match response {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(404, _)) => return Err("No releases have been published yet.".into()),
        Err(e) => return Err(e.to_string()),
    };

    let kind = InstallKind::detect();
    let asset = pick_asset(&json, kind);

    let current = env!("CARGO_PKG_VERSION").to_string();
    let latest = json["tag_name"].as_str().unwrap_or_default().to_string();
    Ok(UpdateInfo {
        available: is_newer(&latest, &current),
        url: json["html_url"].as_str().unwrap_or(REPO_URL).to_string(),
        notes: json["body"].as_str().unwrap_or_default().to_string(),
        install_kind: kind.name(),
        asset,
        current,
        latest,
    })
}

/// Checks and remembers the result so the tray and popup can point at it.
pub fn check_and_store(app: &AppHandle) -> Result<UpdateInfo, String> {
    let info = check()?;
    *app.state::<PendingUpdate>().0.lock().unwrap() = info.available.then(|| info.clone());
    if info.available {
        let _ = app.emit("update-available", &info);
    }
    Ok(info)
}

fn pick_asset(release: &serde_json::Value, kind: InstallKind) -> Option<Asset> {
    let suffix = kind.asset_suffix()?;
    release["assets"].as_array()?.iter().find_map(|a| {
        let name = a["name"].as_str().filter(|n| n.ends_with(suffix))?;
        Some(Asset {
            name: name.to_string(),
            url: a["browser_download_url"].as_str()?.to_string(),
            size: a["size"].as_u64().unwrap_or(0),
            digest: a["digest"].as_str().map(str::to_string),
        })
    })
}

fn parse(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches(['v', 'V'])
        .split(['.', '-', '+'])
        .take(3)
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

fn is_newer(latest: &str, current: &str) -> bool {
    !latest.is_empty() && parse(latest) > parse(current)
}

fn update_dir() -> PathBuf {
    std::env::temp_dir().join("simple-clipboard-update")
}

/// Removes leftovers of a previous update (downloads, the replaced portable exe).
pub fn cleanup() {
    let _ = fs::remove_dir_all(update_dir());
    if let Ok(exe) = std::env::current_exe() {
        let _ = fs::remove_file(exe.with_extension("old"));
    }
}

/// Downloads and installs the latest release. On success the app exits (or restarts).
pub fn install(app: &AppHandle) -> Result<(), String> {
    let info = check()?;
    if !info.available {
        return Err("You already have the latest version.".into());
    }
    let asset = info.asset.ok_or("This release has no download for your installation type.")?;
    let dir = update_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(&asset.name);
    download(&asset, &file, |done, total| {
        let _ = app.emit("update-progress", serde_json::json!({ "downloaded": done, "total": total }));
    })?;
    let _ = app.emit("update-progress", serde_json::json!({ "installing": true }));

    match InstallKind::detect() {
        InstallKind::Installer => {
            Command::new(&file).spawn().map_err(|e| format!("Could not start the installer: {e}"))?;
        }
        InstallKind::Msi => {
            Command::new("msiexec")
                .arg("/i")
                .arg(&file)
                .spawn()
                .map_err(|e| format!("Could not start the installer: {e}"))?;
        }
        InstallKind::Portable => {
            replace_portable_exe(&file)?;
            relaunch_later()?;
        }
        InstallKind::Deb => {
            install_deb(&file)?;
            relaunch_later()?;
        }
        InstallKind::Manual => return Err("Automatic updates are not supported for this installation.".into()),
    }
    app.exit(0);
    Ok(())
}

fn download(asset: &Asset, dest: &Path, progress: impl Fn(u64, u64)) -> Result<(), String> {
    let response = ureq::get(&asset.url)
        .set("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(900))
        .call()
        .map_err(|e| format!("Download failed: {e}"))?;
    let total = response
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(asset.size);

    let mut reader = response.into_reader();
    let mut out = File::create(dest).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    let mut last_report = Instant::now();
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("Download failed: {e}"))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hasher.update(&buf[..n]);
        done += n as u64;
        if last_report.elapsed() > Duration::from_millis(100) {
            last_report = Instant::now();
            progress(done, total);
        }
    }
    out.flush().map_err(|e| e.to_string())?;
    progress(done, total);

    if let Some(expected) = asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")) {
        let actual = format!("{:x}", hasher.finalize());
        if !actual.eq_ignore_ascii_case(expected) {
            let _ = fs::remove_file(dest);
            return Err("The downloaded file is corrupted (checksum mismatch). Please try again.".into());
        }
    }
    Ok(())
}

/// Swaps the running portable exe for the one in the downloaded zip.
/// Windows allows renaming a running executable, just not overwriting it.
fn replace_portable_exe(zip_path: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let staged = exe.with_extension("new");
    let mut archive = zip::ZipArchive::new(File::open(zip_path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Invalid update archive: {e}"))?;
    let index = (0..archive.len())
        .find(|&i| archive.name_for_index(i).is_some_and(|n| n.is_ok_and(|n| n.to_lowercase().ends_with(".exe"))))
        .ok_or("The update archive contains no executable.")?;
    {
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let mut out = File::create(&staged).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }

    let old = exe.with_extension("old");
    let _ = fs::remove_file(&old);
    fs::rename(&exe, &old).map_err(|e| format!("Could not replace the program file: {e}"))?;
    if let Err(e) = fs::rename(&staged, &exe) {
        let _ = fs::rename(&old, &exe);
        return Err(format!("Could not replace the program file: {e}"));
    }
    Ok(())
}

fn install_deb(deb: &Path) -> Result<(), String> {
    let status = Command::new("pkexec")
        .args(["apt-get", "install", "-y", "--allow-downgrades"])
        .arg(deb)
        .status()
        .map_err(|e| format!("Could not run pkexec: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        // Fall back to the desktop's package installer.
        let _ = Command::new("xdg-open").arg(deb).spawn();
        Err("Installation was cancelled or failed. The package was opened in your software installer instead.".into())
    }
}

/// Starts the (updated) app again shortly after this process has exited,
/// so the single-instance check doesn't hand control back to us.
fn relaunch_later() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        Command::new("cmd")
            .raw_arg(format!("/C ping -n 3 127.0.0.1 >nul & start \"\" \"{}\"", exe.display()))
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    {
        Command::new("sh")
            .arg("-c")
            .arg("sleep 2; \"$0\" >/dev/null 2>&1 &")
            .arg(&exe)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{is_newer, pick_asset, InstallKind};

    #[test]
    fn picks_asset_for_install_kind() {
        let release = serde_json::json!({ "assets": [
            { "name": "simple-clipboard_0.2.0_amd64.deb", "browser_download_url": "https://x/deb", "size": 1 },
            { "name": "SimpleClipboard_0.2.0_x64-setup.exe", "browser_download_url": "https://x/setup", "size": 2, "digest": "sha256:ab" },
            { "name": "SimpleClipboard_0.2.0_x64.msi", "browser_download_url": "https://x/msi", "size": 3 },
            { "name": "SimpleClipboard_0.2.0_x64_portable.zip", "browser_download_url": "https://x/zip", "size": 4 },
        ]});
        let url = |kind| pick_asset(&release, kind).map(|a| a.url);
        assert_eq!(url(InstallKind::Installer).as_deref(), Some("https://x/setup"));
        assert_eq!(url(InstallKind::Msi).as_deref(), Some("https://x/msi"));
        assert_eq!(url(InstallKind::Portable).as_deref(), Some("https://x/zip"));
        assert_eq!(url(InstallKind::Deb).as_deref(), Some("https://x/deb"));
        assert_eq!(url(InstallKind::Manual), None);
        assert_eq!(pick_asset(&release, InstallKind::Installer).unwrap().digest.as_deref(), Some("sha256:ab"));
    }

    #[test]
    fn compares_versions() {
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(!is_newer("", "0.1.0"));
    }
}
