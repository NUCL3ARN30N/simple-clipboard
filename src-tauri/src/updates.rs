//! Checks GitHub releases for a newer version.

use std::time::Duration;

use serde::Serialize;

pub const REPO_URL: &str = "https://github.com/NUCL3ARN30N/simple-clipboard";
const API_URL: &str = "https://api.github.com/repos/NUCL3ARN30N/simple-clipboard/releases/latest";

#[derive(Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub available: bool,
    pub url: String,
    pub notes: String,
}

pub fn check() -> Result<UpdateInfo, String> {
    let response = ureq::get(API_URL)
        .set("User-Agent", concat!("simple-clipboard/", env!("CARGO_PKG_VERSION")))
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(10))
        .call();
    let json: serde_json::Value = match response {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(404, _)) => return Err("No releases have been published yet.".into()),
        Err(e) => return Err(e.to_string()),
    };

    let current = env!("CARGO_PKG_VERSION").to_string();
    let latest = json["tag_name"].as_str().unwrap_or_default().to_string();
    Ok(UpdateInfo {
        available: is_newer(&latest, &current),
        url: json["html_url"].as_str().unwrap_or(REPO_URL).to_string(),
        notes: json["body"].as_str().unwrap_or_default().to_string(),
        current,
        latest,
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

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn compares_versions() {
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(!is_newer("", "0.1.0"));
    }
}
