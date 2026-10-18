//! Desktop preferences (session CPU / memory / close behavior). Applied on session start.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::paths::{app_data_dir, write_atomic};

pub const DEFAULT_CPU_COUNT: u32 = 2;
pub const DEFAULT_MEMORY_MB: u32 = 2048;

/// What happens when the user closes the main window.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CloseBehavior {
    /// First close (or after reset): ask minimize vs quit-and-stop.
    #[default]
    Ask,
    /// Hide the window; keep the tray icon and leave containers running.
    MinimizeToTray,
    /// Stop every running container in this session, then quit the app.
    QuitAndStopContainers,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionPrefs {
    pub cpu_count: u32,
    pub memory_mb: u32,
    #[serde(default)]
    pub close_behavior: CloseBehavior,
}

impl Default for SessionPrefs {
    fn default() -> Self {
        Self {
            cpu_count: DEFAULT_CPU_COUNT,
            memory_mb: DEFAULT_MEMORY_MB,
            close_behavior: CloseBehavior::Ask,
        }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
impl SessionPrefs {
    pub fn path() -> PathBuf {
        app_data_dir().join("prefs.json")
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
        let json = serde_json::to_vec_pretty(self).map_err(|e| format!("serialize prefs: {e}"))?;
        write_atomic(&Self::path(), &json)
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
            close_behavior: CloseBehavior::MinimizeToTray,
        };
        let bytes = serde_json::to_vec(&p).unwrap();
        let back: SessionPrefs = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn prefs_default_close_ask_when_missing() {
        let json = br#"{"cpu_count":2,"memory_mb":2048}"#;
        let back: SessionPrefs = serde_json::from_slice(json).unwrap();
        assert_eq!(back.close_behavior, CloseBehavior::Ask);
    }
}
