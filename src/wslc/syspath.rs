//! Put directories that contain `wslcsdk.dll` on the process PATH so LoadLibrary
//! can resolve the bare name used by `wslc-sys`.

#![cfg(windows)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Prefer `preferred` dirs (e.g. auto-downloaded cache), then any system copy.
pub fn prepare_sdk_search_path(preferred: &[PathBuf]) {
    let mut dirs = Vec::new();

    for dir in preferred {
        if dir.join("wslcsdk.dll").is_file() {
            dirs.push(dir.clone());
        }
    }

    for candidate in candidate_dirs() {
        if candidate.join("wslcsdk.dll").is_file() {
            dirs.push(candidate);
        }
    }

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
    unsafe { std::env::set_var("PATH", new_path) };
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.to_path_buf());
        }
    }

    for name in ["wslc.exe", "wslc", "wsl.exe", "wsl"] {
        if let Some(dir) = which_dir(name) {
            dirs.push(dir);
        }
    }

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
