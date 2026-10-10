//! Locate a system-provided `wslcsdk.dll` and put its directory on PATH.
//!
//! We intentionally do **not** ship Microsoft's DLL. LoadLibrary resolves
//! `wslcsdk.dll` from PATH / known WSL install folders so the DLL version
//! matches the host WSL runtime.

#![cfg(windows)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Prepend directories that contain `wslcsdk.dll` to process PATH (best effort).
pub fn prepare_sdk_search_path() {
    let mut dirs = Vec::new();

    for candidate in candidate_dirs() {
        let dll = candidate.join("wslcsdk.dll");
        if dll.is_file() {
            dirs.push(candidate);
        }
    }

    // Dedup while preserving order.
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));

    if dirs.is_empty() {
        return;
    }

    let mut new_path = OsString::new();
    for dir in &dirs {
        if !new_path.is_empty() {
            new_path.push(";");
        }
        new_path.push(dir.as_os_str());
    }
    if let Some(existing) = std::env::var_os("PATH") {
        new_path.push(";");
        new_path.push(existing);
    }
    // Called once at process start before the SDK worker thread is spawned.
    unsafe { std::env::set_var("PATH", new_path) };
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Same folder as this exe (portable drop next to a system copy is fine).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.to_path_buf());
        }
    }

    // Directory of `wslc.exe` / `wsl.exe` when resolvable via PATH.
    for name in ["wslc.exe", "wslc", "wsl.exe", "wsl"] {
        if let Some(dir) = which_dir(name) {
            dirs.push(dir);
        }
    }

    // Well-known WSL install roots.
    for base in [std::env::var_os("ProgramFiles"), std::env::var_os("ProgramW6432")]
        .into_iter()
        .flatten()
    {
        let root = PathBuf::from(base).join("WSL");
        dirs.push(root.clone());
        dirs.push(root.join("tools"));
        dirs.push(root.join("lib"));
        dirs.push(root.join("bin"));
    }

    if let Some(sys) = std::env::var_os("SystemRoot") {
        dirs.push(PathBuf::from(&sys).join("System32"));
        dirs.push(PathBuf::from(sys).join("SysWOW64"));
    }

    // Existing PATH entries (may already contain a NuGet native dir).
    if let Some(path) = std::env::var_os("PATH") {
        for part in std::env::split_paths(&path) {
            if !part.as_os_str().is_empty() {
                dirs.push(part);
            }
        }
    }

    dirs
}

fn which_dir(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(program);
        if candidate.is_file() {
            return Some(dir);
        }
        if Path::new(program).extension().is_none() {
            for ext in [".exe", ".cmd", ".bat"] {
                let with_ext = dir.join(format!("{program}{ext}"));
                if with_ext.is_file() {
                    return Some(dir);
                }
            }
        }
    }
    None
}
