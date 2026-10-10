//! Download and cache `wslcsdk.dll` matched to the host `wslc` / WSL version.
//!
//! WSL ships `wslc.exe` but not the C SDK DLL. On first launch (and whenever the
//! host version changes) we resolve the best [Microsoft.WSL.Containers] NuGet
//! package and extract `runtimes/win-*/native/wslcsdk.dll` under LocalAppData.

#![cfg(windows)]

use std::fs::{self, File};
use std::io::{copy, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context as _, Result};
use serde::{Deserialize, Serialize};

use super::sdk_version::{extract_version, pick_nuget_version};

const NUGET_INDEX: &str =
    "https://api.nuget.org/v3-flatcontainer/microsoft.wsl.containers/index.json";
const INDEX_MAX_AGE: Duration = Duration::from_secs(60 * 60); // 1 hour

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexCache {
    fetched_unix: u64,
    versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActiveStamp {
    host_version: String,
    nuget_version: String,
    arch: String,
}

/// Ensure a host-matched `wslcsdk.dll` is on disk; return its directory for PATH.
pub fn ensure_sdk_dir() -> Result<PathBuf> {
    let host = probe_host_version().unwrap_or_else(|_| "unknown".into());
    let arch = native_arch_folder();
    let versions = nuget_versions().context("list Microsoft.WSL.Containers versions")?;
    let nuget = pick_nuget_version(&host, &versions)
        .ok_or_else(|| anyhow!("no Microsoft.WSL.Containers versions published"))?;

    let dir = sdk_root().join(format!("nuget-{nuget}")).join(arch);
    let dll = dir.join("wslcsdk.dll");
    let stamp_path = sdk_root().join("active.json");

    if dll.is_file() {
        if let Ok(raw) = fs::read_to_string(&stamp_path) {
            if let Ok(stamp) = serde_json::from_str::<ActiveStamp>(&raw) {
                if stamp.host_version == host
                    && stamp.nuget_version == nuget
                    && stamp.arch == arch
                {
                    return Ok(dir);
                }
            }
        }
        // Package already extracted; host string may have changed.
        if nuget_dir_ready(&dir) {
            write_stamp(&stamp_path, &host, &nuget, arch)?;
            return Ok(dir);
        }
    }

    download_nuget_dll(&nuget, arch, &dir)
        .with_context(|| format!("download Microsoft.WSL.Containers {nuget} ({arch})"))?;
    write_stamp(&stamp_path, &host, &nuget, arch)?;
    Ok(dir)
}

fn write_stamp(path: &Path, host: &str, nuget: &str, arch: &str) -> Result<()> {
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

fn nuget_dir_ready(dir: &Path) -> bool {
    dir.join("wslcsdk.dll").is_file()
}

fn sdk_root() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("WSLcDesktop"));
    base.join("WSLcDesktop").join("sdk")
}

fn native_arch_folder() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "win-x64",
        "aarch64" => "win-arm64",
        _ => "win-x64",
    }
}

fn probe_host_version() -> Result<String> {
    for args in [
        ["wslc", "version"].as_slice(),
        ["wslc.exe", "version"].as_slice(),
        ["wsl", "--version"].as_slice(),
        ["wsl.exe", "--version"].as_slice(),
    ] {
        if let Ok(out) = run_hidden(args) {
            if let Some(v) = extract_version(&out) {
                return Ok(v);
            }
        }
    }
    Err(anyhow!(
        "could not read wslc/wsl version (is WSL installed and on PATH?)"
    ))
}

fn run_hidden(args: &[&str]) -> Result<String> {
    let (program, rest) = args
        .split_first()
        .ok_or_else(|| anyhow!("empty command"))?;
    let mut cmd = Command::new(program);
    cmd.args(rest);
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd
        .output()
        .with_context(|| format!("spawn `{program}`"))?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&output.stderr).into_owned();
    }
    if !output.status.success() && text.trim().is_empty() {
        return Err(anyhow!("`{program}` exited {}", output.status));
    }
    Ok(text)
}

fn nuget_versions() -> Result<Vec<String>> {
    let cache_path = sdk_root().join("nuget-index.json");
    if let Ok(raw) = fs::read_to_string(&cache_path) {
        if let Ok(cache) = serde_json::from_str::<IndexCache>(&raw) {
            let age = now_unix().saturating_sub(cache.fetched_unix);
            if age < INDEX_MAX_AGE.as_secs() && !cache.versions.is_empty() {
                return Ok(cache.versions);
            }
        }
    }

    let body: serde_json::Value = ureq::get(NUGET_INDEX)
        .call()
        .context("GET nuget index")?
        .into_json()
        .context("parse nuget index")?;
    let versions = body
        .get("versions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if versions.is_empty() {
        return Err(anyhow!("empty nuget version list"));
    }

    fs::create_dir_all(sdk_root())?;
    let cache = IndexCache {
        fetched_unix: now_unix(),
        versions: versions.clone(),
    };
    fs::write(&cache_path, serde_json::to_string_pretty(&cache)?)?;
    Ok(versions)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn download_nuget_dll(version: &str, arch: &str, out_dir: &Path) -> Result<()> {
    fs::create_dir_all(out_dir)?;
    let url = format!(
        "https://api.nuget.org/v3-flatcontainer/microsoft.wsl.containers/{version}/microsoft.wsl.containers.{version}.nupkg"
    );
    let tmp_nupkg = out_dir.join(format!("microsoft.wsl.containers.{version}.nupkg"));
    {
        let resp = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?;
        let mut reader = resp.into_reader();
        let mut file = File::create(&tmp_nupkg)?;
        copy(&mut reader, &mut file)?;
        file.flush()?;
    }

    let file = File::open(&tmp_nupkg)?;
    let mut archive = zip::ZipArchive::new(file).context("open nupkg as zip")?;
    let member = format!("runtimes/{arch}/native/wslcsdk.dll");
    let mut entry = archive
        .by_name(&member)
        .with_context(|| format!("nupkg missing {member}"))?;
    let dll_path = out_dir.join("wslcsdk.dll");
    let mut out = File::create(&dll_path)?;
    copy(&mut entry, &mut out)?;
    out.flush()?;

    let _ = fs::remove_file(&tmp_nupkg);

    if !dll_path.is_file() {
        return Err(anyhow!("extracted dll missing at {}", dll_path.display()));
    }
    Ok(())
}
