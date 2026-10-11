//! On-disk cache for extracted `wslcsdk.dll` under `%LOCALAPPDATA%\WSLcDesktop\sdk`.

#![cfg(windows)]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::paths;
use super::sdk_version::cache_compatible;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveStamp {
    pub host_version: String,
    pub nuget_version: String,
    pub arch: String,
}

pub fn root() -> PathBuf {
    paths::app_data_dir().join("sdk")
}

pub fn native_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "win-x64",
        "aarch64" => "win-arm64",
        _ => "win-x64",
    }
}

pub fn stamp_path() -> PathBuf {
    root().join("active.json")
}

pub fn package_dir(nuget: &str, arch: &str) -> PathBuf {
    root().join(format!("nuget-{nuget}")).join(arch)
}

pub fn dll_ready(dir: &Path) -> bool {
    dir.join("wslcsdk.dll").is_file()
}

/// Look up the active stamp's DLL when present and (optionally) host-compatible.
pub fn lookup(host: Option<&str>, arch: &str) -> Option<(PathBuf, ActiveStamp)> {
    let raw = fs::read_to_string(stamp_path()).ok()?;
    let stamp: ActiveStamp = serde_json::from_str(&raw).ok()?;
    if stamp.arch != arch {
        return None;
    }
    if let Some(host) = host
        && !cache_compatible(host, &stamp.nuget_version)
    {
        return None;
    }
    let dir = package_dir(&stamp.nuget_version, arch);
    dll_ready(&dir).then_some((dir, stamp))
}

pub fn write_stamp(host: &str, nuget: &str, arch: &str) -> Result<()> {
    let path = stamp_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let stamp = ActiveStamp {
        host_version: host.to_string(),
        nuget_version: nuget.to_string(),
        arch: arch.to_string(),
    };
    fs::write(path, serde_json::to_string_pretty(&stamp)?)?;
    Ok(())
}

pub fn clear_package(nuget: &str, arch: &str) {
    let dll = package_dir(nuget, arch).join("wslcsdk.dll");
    let _ = fs::remove_file(dll);
    let _ = fs::remove_file(stamp_path());
}
