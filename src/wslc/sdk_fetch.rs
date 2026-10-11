//! Download `wslcsdk.dll` from NuGet when the local cache cannot serve the host.
//!
//! Cache lookup / stamps live in [`super::sdk_cache`]; host probing in
//! [`super::wsl_probe`]. This module only lists packages, downloads, and extracts.

#![cfg(windows)]

use std::fs::{self, File};
use std::io::{copy, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context as _, Result};
use serde::{Deserialize, Serialize};

use super::sdk_cache;
use super::sdk_version::pick_nuget_version;
use super::wsl_probe;

const NUGET_INDEX: &str =
    "https://api.nuget.org/v3-flatcontainer/microsoft.wsl.containers/index.json";
const INDEX_MAX_AGE: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone)]
pub enum FetchEvent {
    Status(String),
    Log(String),
    Progress(f32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexCache {
    fetched_unix: u64,
    versions: Vec<String>,
}

/// Ensure a host-matched `wslcsdk.dll` is on disk; return its directory for PATH.
///
/// Progress values are 0–100 for this download/cache stage.
pub fn ensure_sdk_dir(
    force_redownload: bool,
    mut on_event: impl FnMut(FetchEvent),
) -> Result<PathBuf> {
    let mut emit = |ev: FetchEvent| on_event(ev);

    emit(FetchEvent::Status("Detecting WSL version…".into()));
    emit(FetchEvent::Progress(5.0));
    let host = match wsl_probe::host_version() {
        Ok(v) => {
            emit(FetchEvent::Log(format!("Host version: {v}")));
            v
        }
        Err(err) => {
            emit(FetchEvent::Log(format!("Could not read WSL version: {err:#}")));
            emit(FetchEvent::Log(
                "Install or update WSL (`wsl --install` / `wsl --update`), then retry.".into(),
            ));
            "unknown".into()
        }
    };

    let arch = sdk_cache::native_arch();
    emit(FetchEvent::Log(format!("Native arch folder: {arch}")));
    emit(FetchEvent::Progress(12.0));

    if !force_redownload
        && let Some((dir, stamp)) = sdk_cache::lookup(Some(&host), arch)
    {
        if stamp.host_version != host {
            emit(FetchEvent::Log(format!(
                "Host probe drifted ({} → {}); keeping cached SDK {}",
                stamp.host_version, host, stamp.nuget_version
            )));
            let _ = sdk_cache::write_stamp(&host, &stamp.nuget_version, arch);
        }
        return cache_hit(&mut emit, &dir);
    }

    emit(FetchEvent::Status(
        "Listing Microsoft.WSL.Containers on NuGet…".into(),
    ));
    let versions = match list_nuget_versions(&mut emit) {
        Ok(versions) => versions,
        Err(err) => {
            if !force_redownload
                && let Some((dir, _)) = sdk_cache::lookup(None, arch)
            {
                emit(FetchEvent::Log(format!(
                    "NuGet unavailable ({err:#}); reusing active SDK cache"
                )));
                return cache_hit(&mut emit, &dir);
            }
            return Err(err).context("list Microsoft.WSL.Containers versions");
        }
    };
    emit(FetchEvent::Log(format!(
        "NuGet published {} package version(s)",
        versions.len()
    )));
    emit(FetchEvent::Progress(22.0));

    let nuget = pick_nuget_version(&host, &versions)
        .ok_or_else(|| anyhow!("no Microsoft.WSL.Containers versions published"))?;
    emit(FetchEvent::Log(format!(
        "Selected NuGet package version: {nuget} (host {host})"
    )));

    let dir = sdk_cache::package_dir(&nuget, arch);
    if force_redownload && sdk_cache::dll_ready(&dir) {
        emit(FetchEvent::Log(format!(
            "Retry requested — clearing cached DLL for {nuget}"
        )));
        sdk_cache::clear_package(&nuget, arch);
    }

    if sdk_cache::dll_ready(&dir) {
        emit(FetchEvent::Status("Refreshing SDK stamp…".into()));
        emit(FetchEvent::Log(format!(
            "Reusing extracted DLL at {}",
            dir.join("wslcsdk.dll").display()
        )));
        sdk_cache::write_stamp(&host, &nuget, arch)?;
        emit(FetchEvent::Progress(100.0));
        return Ok(dir);
    }

    emit(FetchEvent::Status(format!(
        "Downloading Microsoft.WSL.Containers {nuget}…"
    )));
    emit(FetchEvent::Progress(28.0));
    download_nuget_dll(&nuget, arch, &dir, &mut emit)
        .with_context(|| format!("download Microsoft.WSL.Containers {nuget} ({arch})"))?;
    sdk_cache::write_stamp(&host, &nuget, arch)?;
    emit(FetchEvent::Status("SDK installed".into()));
    emit(FetchEvent::Progress(100.0));
    Ok(dir)
}

fn cache_hit(emit: &mut dyn FnMut(FetchEvent), dir: &Path) -> Result<PathBuf> {
    emit(FetchEvent::Status("Using cached SDK".into()));
    emit(FetchEvent::Log(format!(
        "Cache hit: {}",
        dir.join("wslcsdk.dll").display()
    )));
    emit(FetchEvent::Progress(100.0));
    Ok(dir.to_path_buf())
}

fn list_nuget_versions(emit: &mut dyn FnMut(FetchEvent)) -> Result<Vec<String>> {
    let cache_path = sdk_cache::root().join("nuget-index.json");
    let cached = fs::read_to_string(&cache_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<IndexCache>(&raw).ok())
        .filter(|cache| !cache.versions.is_empty());

    if let Some(ref cache) = cached {
        let age = now_unix().saturating_sub(cache.fetched_unix);
        if age < INDEX_MAX_AGE.as_secs() {
            emit(FetchEvent::Log(
                "Using cached NuGet version index (< 1h old)".into(),
            ));
            return Ok(cache.versions.clone());
        }
    }

    emit(FetchEvent::Log(format!("GET {NUGET_INDEX}")));
    let fetched = (|| -> Result<Vec<String>> {
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
        Ok(versions)
    })();

    match fetched {
        Ok(versions) => {
            fs::create_dir_all(sdk_cache::root())?;
            let cache = IndexCache {
                fetched_unix: now_unix(),
                versions: versions.clone(),
            };
            fs::write(&cache_path, serde_json::to_string_pretty(&cache)?)?;
            Ok(versions)
        }
        Err(err) => {
            if let Some(cache) = cached {
                emit(FetchEvent::Log(format!(
                    "NuGet index refresh failed ({err:#}); using stale cached index"
                )));
                Ok(cache.versions)
            } else {
                Err(err)
            }
        }
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn download_nuget_dll(
    version: &str,
    arch: &str,
    out_dir: &Path,
    emit: &mut dyn FnMut(FetchEvent),
) -> Result<()> {
    fs::create_dir_all(out_dir)?;
    let url = format!(
        "https://api.nuget.org/v3-flatcontainer/microsoft.wsl.containers/{version}/microsoft.wsl.containers.{version}.nupkg"
    );
    let tmp_nupkg = out_dir.join(format!("microsoft.wsl.containers.{version}.nupkg"));
    emit(FetchEvent::Log(format!("GET {url}")));
    {
        let resp = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?;
        let total = resp
            .header("Content-Length")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let mut reader = resp.into_reader();
        let mut file = File::create(&tmp_nupkg)?;
        let mut buf = [0u8; 64 * 1024];
        let mut written: u64 = 0;
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            written += n as u64;
            if total > 0 {
                let frac = written as f32 / total as f32;
                emit(FetchEvent::Progress(28.0 + frac * 47.0));
            } else if written % (512 * 1024) < 64 * 1024 {
                emit(FetchEvent::Log(format!(
                    "Downloaded {} KB…",
                    written / 1024
                )));
            }
        }
        file.flush()?;
        emit(FetchEvent::Log(format!(
            "Downloaded nupkg ({} KB)",
            written / 1024
        )));
    }

    emit(FetchEvent::Status("Extracting wslcsdk.dll…".into()));
    emit(FetchEvent::Progress(78.0));
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
    emit(FetchEvent::Log(format!(
        "Extracted {}",
        dll_path.display()
    )));
    emit(FetchEvent::Progress(95.0));
    Ok(())
}
