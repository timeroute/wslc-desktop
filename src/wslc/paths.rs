//! Shared desktop data directory (`%LOCALAPPDATA%\WSLcDesktop`).

use std::fs;
use std::path::{Path, PathBuf};

pub fn app_data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\WslcData"));
    base.join("WSLcDesktop")
}

/// Write `bytes` via a sibling temp file, then rename over `path`.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes).map_err(|e| format!("write tmp: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("replace file: {e}"))?;
    Ok(())
}
