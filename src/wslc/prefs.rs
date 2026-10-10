//! Desktop preferences (session CPU / memory). Applied on session start.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const DEFAULT_CPU_COUNT: u32 = 2;
pub const DEFAULT_MEMORY_MB: u32 = 2048;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionPrefs {
    pub cpu_count: u32,
    pub memory_mb: u32,
}

impl Default for SessionPrefs {
    fn default() -> Self {
        Self {
            cpu_count: DEFAULT_CPU_COUNT,
            memory_mb: DEFAULT_MEMORY_MB,
        }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
impl SessionPrefs {
    pub fn path() -> PathBuf {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\WslcData"));
        base.join("WSLcDesktop").join("prefs.json")
    }

    pub fn load() -> Self {
        let Ok(bytes) = fs::read(Self::path()) else {
            return Self::default();
        };
        match serde_json::from_slice::<SessionPrefs>(&bytes) {
            Ok(mut p) => {
                if p.cpu_count == 0 {
                    p.cpu_count = DEFAULT_CPU_COUNT;
                }
                if p.memory_mb < 256 {
                    p.memory_mb = DEFAULT_MEMORY_MB;
                }
                p
            }
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create prefs dir: {e}"))?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(|e| format!("serialize prefs: {e}"))?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| format!("write prefs tmp: {e}"))?;
        fs::rename(&tmp, path).map_err(|e| format!("replace prefs: {e}"))?;
        Ok(())
    }

    pub fn clamp(mut self) -> Self {
        self.cpu_count = self.cpu_count.clamp(1, 16);
        self.memory_mb = self.memory_mb.clamp(512, 32768);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefs_roundtrip() {
        let p = SessionPrefs {
            cpu_count: 4,
            memory_mb: 4096,
        };
        let bytes = serde_json::to_vec(&p).unwrap();
        let back: SessionPrefs = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, p);
    }
}
