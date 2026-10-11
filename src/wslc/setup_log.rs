//! Append-only SDK setup log under the app data directory.
//!
//! File: `%LOCALAPPDATA%\WSLcDesktop\logs\sdk-setup.log`

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::paths::app_data_dir;

const MAX_BYTES: u64 = 256 * 1024;
const KEEP_TAIL: usize = 128 * 1024;

pub fn log_dir() -> PathBuf {
    app_data_dir().join("logs")
}

pub fn log_file_path() -> PathBuf {
    log_dir().join("sdk-setup.log")
}

/// Start a setup attempt. Trims the file when it grows past [`MAX_BYTES`].
pub fn begin_session() -> Result<(), String> {
    begin_session_at(&log_file_path())
}

pub fn begin_session_at(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create log dir: {e}"))?;
    }
    trim_if_needed(path)?;
    append_raw(path, &format!("\n===== sdk setup {} =====\n", utc_stamp()))
}

pub fn append_line(line: &str) -> Result<(), String> {
    append_line_at(&log_file_path(), line)
}

pub fn append_line_at(path: &Path, line: &str) -> Result<(), String> {
    if line.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create log dir: {e}"))?;
    }
    let stamp = utc_stamp();
    let mut out = String::new();
    for part in line.split('\n') {
        let part = part.trim_end_matches('\r');
        if part.is_empty() {
            continue;
        }
        out.push_str(&stamp);
        out.push(' ');
        out.push_str(part);
        out.push('\n');
    }
    if out.is_empty() {
        return Ok(());
    }
    append_raw(path, &out)
}

/// Open the log directory in the system file manager.
pub fn open_log_dir() -> Result<(), String> {
    let dir = log_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create log dir: {e}"))?;
    spawn_folder(&dir)
}

fn spawn_folder(dir: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("explorer")
            .arg(dir)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("explorer: {e}"))
    }
    #[cfg(not(windows))]
    {
        Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("xdg-open: {e}"))
    }
}

fn trim_if_needed(path: &Path) -> Result<(), String> {
    let Ok(meta) = fs::metadata(path) else {
        return Ok(());
    };
    if meta.len() <= MAX_BYTES {
        return Ok(());
    }
    let bytes = fs::read(path).map_err(|e| format!("read log: {e}"))?;
    let start = bytes.len().saturating_sub(KEEP_TAIL);
    let start = bytes[start..]
        .iter()
        .position(|b| *b == b'\n')
        .map(|i| start + i + 1)
        .unwrap_or(start);
    fs::write(path, &bytes[start..]).map_err(|e| format!("trim log: {e}"))
}

fn append_raw(path: &Path, text: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("open log: {e}"))?;
    file.write_all(text.as_bytes())
        .map_err(|e| format!("write log: {e}"))?;
    Ok(())
}

fn utc_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    let hh = tod / 3600;
    let mm = (tod % 3600) / 60;
    let ss = tod % 60;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Days since Unix epoch → civil (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_known_days() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
    }

    #[test]
    fn append_and_rotate() {
        let dir = std::env::temp_dir().join(format!("wslc-setup-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sdk-setup.log");

        begin_session_at(&path).unwrap();
        append_line_at(&path, "hello").unwrap();
        append_line_at(&path, "line-a\nline-b").unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("===== sdk setup "));
        assert!(text.contains("hello"));
        assert!(text.contains("line-a"));
        assert!(text.contains("line-b"));

        let big = vec![b'x'; (MAX_BYTES as usize) + 1024];
        fs::write(&path, &big).unwrap();
        begin_session_at(&path).unwrap();
        let trimmed = fs::metadata(&path).unwrap().len();
        assert!(trimmed < MAX_BYTES, "trimmed len {trimmed}");
        let _ = fs::remove_dir_all(&dir);
    }
}
