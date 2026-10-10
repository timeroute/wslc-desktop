use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn unique_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    format!("{prefix}-{nanos}-{}", std::process::id())
}

pub fn storage_path(name: &str) -> PathBuf {
    PathBuf::from(format!(r"C:\WslcData\{name}"))
}
