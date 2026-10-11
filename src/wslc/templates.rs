//! Saved container-create forms.
//!
//! Path: `%LOCALAPPDATA%\WSLcDesktop\templates.json` (next to prefs and catalog).

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::paths::{app_data_dir, write_atomic};

pub const TEMPLATE_VERSION: u32 = 1;
pub const MAX_TEMPLATE_NAME_CHARS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertKind {
    Created,
    Updated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateEditError {
    EmptyName,
    NameTooLong,
    NameTaken,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerTemplate {
    pub name: String,
    #[serde(default)]
    pub image: String,
    /// Optional container name field from the create form (not the template name).
    #[serde(default)]
    pub container_name: String,
    #[serde(default)]
    pub ports: String,
    #[serde(default)]
    pub bind_mounts: String,
    #[serde(default)]
    pub named_volumes: String,
    #[serde(default)]
    pub auto_remove: bool,
    #[serde(default)]
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TemplateStore {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub templates: Vec<ContainerTemplate>,
}

fn default_version() -> u32 {
    TEMPLATE_VERSION
}

impl Default for TemplateStore {
    fn default() -> Self {
        Self {
            version: TEMPLATE_VERSION,
            templates: Vec::new(),
        }
    }
}

impl TemplateStore {
    pub fn path() -> PathBuf {
        app_data_dir().join("templates.json")
    }

    pub fn load() -> Self {
        Self::load_from(&Self::path())
    }

    pub fn load_from(path: &std::path::Path) -> Self {
        let Ok(bytes) = fs::read(path) else {
            return Self::default();
        };
        match serde_json::from_slice::<TemplateStore>(&bytes) {
            Ok(mut store) => {
                store.version = TEMPLATE_VERSION;
                store.templates.retain(|t| !t.name.trim().is_empty());
                store
            }
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<(), String> {
        let json =
            serde_json::to_vec_pretty(self).map_err(|e| format!("serialize templates: {e}"))?;
        write_atomic(path, &json)
    }

    pub fn upsert(
        &mut self,
        mut template: ContainerTemplate,
    ) -> Result<(UpsertKind, String), TemplateEditError> {
        template.name = sanitize_name(&template.name);
        validate_name(&template.name)?;
        let name = template.name.clone();
        if let Some(existing) = self.templates.iter_mut().find(|t| t.name == template.name) {
            *existing = template;
            Ok((UpsertKind::Updated, name))
        } else {
            self.templates.insert(0, template);
            Ok((UpsertKind::Created, name))
        }
    }

    pub fn rename(&mut self, from: &str, to: &str) -> Result<String, TemplateEditError> {
        let to = sanitize_name(to);
        validate_name(&to)?;
        if to != from && self.templates.iter().any(|t| t.name == to) {
            return Err(TemplateEditError::NameTaken);
        }
        let Some(item) = self.templates.iter_mut().find(|t| t.name == from) else {
            return Err(TemplateEditError::Missing);
        };
        item.name = to.clone();
        Ok(to)
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.templates.len();
        self.templates.retain(|t| t.name != name);
        self.templates.len() != before
    }
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}

fn validate_name(name: &str) -> Result<(), TemplateEditError> {
    if name.is_empty() {
        return Err(TemplateEditError::EmptyName);
    }
    if name.chars().count() > MAX_TEMPLATE_NAME_CHARS {
        return Err(TemplateEditError::NameTooLong);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str) -> ContainerTemplate {
        ContainerTemplate {
            name: name.into(),
            image: "nginx:alpine".into(),
            container_name: "web".into(),
            ports: "8080:80".into(),
            bind_mounts: r"C:\data:/data".into(),
            named_volumes: "data:/data".into(),
            auto_remove: true,
            command: "nginx -g 'daemon off;'".into(),
        }
    }

    #[test]
    fn roundtrip_and_edits() {
        let dir = std::env::temp_dir().join(format!("wslc-templates-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("templates.json");

        let mut store = TemplateStore::default();
        assert_eq!(
            store.upsert(sample("nginx")).unwrap().0,
            UpsertKind::Created
        );
        assert_eq!(
            store.upsert(sample("nginx")).unwrap().0,
            UpsertKind::Updated
        );
        assert_eq!(store.templates.len(), 1);
        assert!(store.templates[0].auto_remove);
        store.upsert(sample("redis")).unwrap();
        assert_eq!(store.templates[0].name, "redis");
        store.rename("redis", "cache").unwrap();
        assert!(store.rename("cache", "nginx").is_err());
        assert!(store.remove("cache"));
        assert!(!store.remove("cache"));
        assert_eq!(
            store.upsert(sample("  ")).unwrap_err(),
            TemplateEditError::EmptyName
        );

        store.save_to(&path).unwrap();
        let loaded = TemplateStore::load_from(&path);
        assert_eq!(loaded.templates.len(), 1);
        assert_eq!(loaded.templates[0].image, "nginx:alpine");
        assert_eq!(loaded.templates[0].ports, "8080:80");
        assert_eq!(loaded.templates[0].bind_mounts, r"C:\data:/data");
        assert_eq!(loaded.templates[0].named_volumes, "data:/data");
        let _ = fs::remove_dir_all(&dir);
    }
}
