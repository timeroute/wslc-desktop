//! Download and cache `wslcsdk.dll` matched to the host `wslc` / WSL version.
//!
//! WSL ships `wslc.exe` but not the C SDK DLL. On first launch (and whenever the
//! host version changes) we resolve the best [Microsoft.WSL.Containers] NuGet
//! package and extract `runtimes/win-*/native/wslcsdk.dll` under LocalAppData.

#![cfg(windows)]

use std::fs::{self, File};
use std::io::{copy, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context as _, Result};
use serde::{Deserialize, Serialize};

use super::sdk_version::{extract_version, pick_nuget_version};

const NUGET_INDEX: &str =
    "https://api.nuget.org/v3-flatcontainer/microsoft.wsl.containers/index.json";
const INDEX_MAX_AGE: Duration = Duration::from_secs(60 * 60); // 1 hour

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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActiveStamp {
    host_version: String,
    nuget_version: String,
    arch: String,
}

/// Ensure a host-matched `wslcsdk.dll` is on disk; return its directory for PATH.
///
/// Progress values are 0–100 for this download/cache stage.
pub fn ensure_sdk_dir(
    force_redownload: bool,
    mut on_event: impl FnMut(FetchEvent),
) -> Result<PathBuf> {
    let mut emit = |ev: FetchEvent| on_event(ev);

    emit(FetchEvent::Status("Detecting WSL / wslc version…".into()));
    emit(FetchEvent::Progress(5.0));
    let host = match probe_host_version() {
        Ok(v) => {
            emit(FetchEvent::Log(format!("Host version: {v}")));
            v
        }
        Err(err) => {
            emit(FetchEvent::Log(format!(
                "Could not read wslc/wsl version: {err:#}"
            )));
            emit(FetchEvent::Log(
                "Install or update WSL (`wsl --install` / `wsl --update`), ensure it is on PATH, then retry."
                    .into(),
            ));
            "unknown".into()
        }
    };

    let arch = native_arch_folder();
    emit(FetchEvent::Log(format!("Native arch folder: {arch}")));
    emit(FetchEvent::Progress(12.0));

    let stamp_path = sdk_root().join("active.json");

    // Prefer a matching on-disk DLL before touching NuGet (offline / stale index).
    if !force_redownload {
        if let Some(dir) = try_active_sdk_dir(&stamp_path, Some(&host), arch) {
            emit(FetchEvent::Status("Using cached SDK".into()));
            emit(FetchEvent::Log(format!(
                "Cache hit (active stamp): {}",
                dir.join("wslcsdk.dll").display()
            )));
            emit(FetchEvent::Progress(100.0));
            return Ok(dir);
        }
    }

    emit(FetchEvent::Status("Listing Microsoft.WSL.Containers on NuGet…".into()));
    let versions = match nuget_versions(&mut emit) {
        Ok(versions) => versions,
        Err(err) => {
            // Last resort: any previously activated DLL, even if host string drifted.
            if !force_redownload {
                if let Some(dir) = try_active_sdk_dir(&stamp_path, None, arch) {
                    emit(FetchEvent::Log(format!(
                        "NuGet unavailable ({err:#}); reusing active SDK cache"
                    )));
                    emit(FetchEvent::Status("Using cached SDK".into()));
                    emit(FetchEvent::Progress(100.0));
                    return Ok(dir);
                }
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

    let dir = sdk_root().join(format!("nuget-{nuget}")).join(arch);
    let dll = dir.join("wslcsdk.dll");

    if force_redownload && dll.is_file() {
        emit(FetchEvent::Log(format!(
            "Retry requested — removing cached DLL at {}",
            dll.display()
        )));
        let _ = fs::remove_file(&dll);
        let _ = fs::remove_file(&stamp_path);
    }

    if dll.is_file() {
        if let Ok(raw) = fs::read_to_string(&stamp_path) {
            if let Ok(stamp) = serde_json::from_str::<ActiveStamp>(&raw) {
                if stamp.host_version == host
                    && stamp.nuget_version == nuget
                    && stamp.arch == arch
                {
                    emit(FetchEvent::Status("Using cached SDK".into()));
                    emit(FetchEvent::Log(format!(
                        "Cache hit: {}",
                        dll.display()
                    )));
                    emit(FetchEvent::Progress(100.0));
                    return Ok(dir);
                }
            }
        }
        // Package already extracted; host string may have changed.
        if nuget_dir_ready(&dir) {
            emit(FetchEvent::Status("Refreshing SDK stamp…".into()));
            emit(FetchEvent::Log(format!(
                "Reusing extracted DLL at {}",
                dll.display()
            )));
            write_stamp(&stamp_path, &host, &nuget, arch)?;
            emit(FetchEvent::Progress(100.0));
            return Ok(dir);
        }
    }

    emit(FetchEvent::Status(format!(
        "Downloading Microsoft.WSL.Containers {nuget}…"
    )));
    emit(FetchEvent::Progress(28.0));
    download_nuget_dll(&nuget, arch, &dir, &mut emit)
        .with_context(|| format!("download Microsoft.WSL.Containers {nuget} ({arch})"))?;
    write_stamp(&stamp_path, &host, &nuget, arch)?;
    emit(FetchEvent::Status("SDK installed".into()));
    emit(FetchEvent::Progress(100.0));
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

/// Raw `wsl --version` text, if the command ran and printed anything.
pub(crate) fn probe_wsl_version_output() -> Option<String> {
    for args in [
        ["wsl", "--version"].as_slice(),
        ["wsl.exe", "--version"].as_slice(),
    ] {
        if let Ok(out) = run_hidden(args) {
            if !out.trim().is_empty() {
                return Some(out);
            }
        }
    }
    None
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

fn try_active_sdk_dir(
    stamp_path: &Path,
    host: Option<&str>,
    arch: &str,
) -> Option<PathBuf> {
    let raw = fs::read_to_string(stamp_path).ok()?;
    let stamp: ActiveStamp = serde_json::from_str(&raw).ok()?;
    if stamp.arch != arch {
        return None;
    }
    if let Some(host) = host {
        if stamp.host_version != host {
            return None;
        }
    }
    let dir = sdk_root()
        .join(format!("nuget-{}", stamp.nuget_version))
        .join(arch);
    nuget_dir_ready(&dir).then_some(dir)
}

fn nuget_versions(emit: &mut dyn FnMut(FetchEvent)) -> Result<Vec<String>> {
    let cache_path = sdk_root().join("nuget-index.json");
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
            fs::create_dir_all(sdk_root())?;
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
                // Map download into 28–75%.
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
