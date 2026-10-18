//! App version + GitHub Releases latest check (open browser for updates).

use std::process::Command;

use serde::Deserialize;

use super::sdk_version::SemVer;

/// Built from `Cargo.toml` `[package].version`.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Repository homepage from `Cargo.toml` `[package].repository`.
pub const GITHUB_REPO_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// Project site from `Cargo.toml` `[package].homepage`.
pub const HOMEPAGE_URL: &str = env!("CARGO_PKG_HOMEPAGE");

const RELEASES_URL: &str = "https://github.com/timeroute/wslc-desktop/releases";
const LATEST_API: &str =
    "https://api.github.com/repos/timeroute/wslc-desktop/releases/latest";

/// Result of comparing the running build to GitHub `releases/latest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    /// Installed version is newest, or equal to the latest tag.
    UpToDate { latest: String },
    /// A newer non-draft release exists.
    Available { latest: String, url: String },
}

#[derive(Debug, Deserialize)]
struct LatestReleaseJson {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// Strip a leading `v`/`V` so `v0.1.2` parses like `0.1.2`.
pub fn normalize_version_tag(tag: &str) -> &str {
    let t = tag.trim();
    t.strip_prefix('v')
        .or_else(|| t.strip_prefix('V'))
        .unwrap_or(t)
}

/// Compare `current` (e.g. `CARGO_PKG_VERSION`) to a release tag.
pub fn compare_to_tag(current: &str, tag: &str) -> Option<std::cmp::Ordering> {
    let cur = SemVer::parse(normalize_version_tag(current))?;
    let remote = SemVer::parse(normalize_version_tag(tag))?;
    Some(cur.cmp(&remote))
}

/// Fetch GitHub `releases/latest` and compare to `current_version`.
pub fn check_latest(current_version: &str) -> Result<UpdateStatus, String> {
    let release = fetch_latest_release()?;
    if release.draft || release.prerelease {
        return Err("latest release is draft/prerelease".into());
    }
    let latest = normalize_version_tag(&release.tag_name).to_string();
    let Some(order) = compare_to_tag(current_version, &release.tag_name) else {
        return Err(format!("unparseable version tag: {}", release.tag_name));
    };
    match order {
        std::cmp::Ordering::Less => Ok(UpdateStatus::Available {
            latest,
            url: release.html_url,
        }),
        std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => {
            Ok(UpdateStatus::UpToDate { latest })
        }
    }
}

fn fetch_latest_release() -> Result<LatestReleaseJson, String> {
    let resp = ureq::get(LATEST_API)
        .set("User-Agent", "wslc-desktop")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("GitHub API: {e}"))?;
    resp.into_json::<LatestReleaseJson>()
        .map_err(|e| format!("GitHub API JSON: {e}"))
}

/// Releases list page (fallback when a specific release URL is unavailable).
pub fn releases_url() -> &'static str {
    RELEASES_URL
}

/// Open an http(s) URL in the default browser.
pub fn open_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("refusing to open non-http URL".into());
    }
    #[cfg(windows)]
    {
        // `start` is a cmd builtin; empty title arg avoids swallowing the URL.
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("open URL: {e}"))
    }
    #[cfg(not(windows))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("open URL: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_v_prefix() {
        assert_eq!(normalize_version_tag("v0.1.2"), "0.1.2");
        assert_eq!(normalize_version_tag("V1.0.0"), "1.0.0");
        assert_eq!(normalize_version_tag("0.1.2"), "0.1.2");
    }

    #[test]
    fn compares_release_tags() {
        assert_eq!(
            compare_to_tag("0.1.2", "v0.1.2"),
            Some(std::cmp::Ordering::Equal)
        );
        assert_eq!(
            compare_to_tag("0.1.1", "v0.1.2"),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(
            compare_to_tag("0.2.0", "v0.1.9"),
            Some(std::cmp::Ordering::Greater)
        );
        assert!(compare_to_tag("0.1.2", "not-a-tag").is_none());
    }

    #[test]
    fn package_metadata_urls_are_https() {
        assert!(GITHUB_REPO_URL.starts_with("https://"));
        assert!(HOMEPAGE_URL.starts_with("https://"));
        assert!(!APP_VERSION.is_empty());
    }
}
