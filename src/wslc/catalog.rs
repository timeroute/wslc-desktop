//! Persist desktop-tracked containers / volumes across app restarts.
//!
//! Stored outside the WSLC session root (which must stay empty for CreateSession).
//! Path: `%LOCALAPPDATA%\WSLcDesktop\catalog.json`

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::paths::{app_data_dir, write_atomic};
use super::types::VolumeInfo;

pub const CATALOG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    #[serde(default)]
    pub ports: String,
    #[serde(default)]
    pub volumes: String,
    #[serde(default)]
    pub auto_remove: bool,
    #[serde(default)]
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Catalog {
    pub version: u32,
    #[serde(default)]
    pub containers: Vec<PersistedContainer>,
    #[serde(default)]
    pub volumes: Vec<VolumeInfo>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self {
            version: CATALOG_VERSION,
            containers: Vec::new(),
            volumes: Vec::new(),
        }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Catalog {
    pub fn catalog_path() -> PathBuf {
        app_data_dir().join("catalog.json")
    }

    pub fn load() -> Self {
        let path = Self::catalog_path();
        let Ok(bytes) = fs::read(&path) else {
            return Self::default();
        };
        match serde_json::from_slice::<Catalog>(&bytes) {
            Ok(mut cat) => {
                cat.version = CATALOG_VERSION;
                cat
            }
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let json =
            serde_json::to_vec_pretty(self).map_err(|e| format!("serialize catalog: {e}"))?;
        write_atomic(&Self::catalog_path(), &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_catalog_json() {
        let cat = Catalog {
            version: CATALOG_VERSION,
            containers: vec![PersistedContainer {
                id: "abc123".into(),
                name: "web".into(),
                image: "nginx:alpine".into(),
                ports: "8080:80".into(),
                volumes: String::new(),
                auto_remove: false,
                created: "Just now".into(),
            }],
            volumes: vec![VolumeInfo {
                name: "data".into(),
                driver: "vhd".into(),
                mountpoint: "session:data".into(),
                created: "Just now".into(),
                size: "1.0 GB".into(),
            }],
        };
        let bytes = serde_json::to_vec(&cat).unwrap();
        let back: Catalog = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, cat);
    }
}
