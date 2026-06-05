use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct PlatformEntry {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub status: String,
    pub adapter_version: String,
    pub doc_url: Option<String>,
    #[serde(default)]
    pub surfaces: Vec<String>,
    pub paths: Option<PlatformPaths>,
    pub optional_api: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlatformPaths {
    pub windows: Option<Vec<String>>,
    pub macos: Option<Vec<String>>,
    pub linux: Option<Vec<String>>,
    #[serde(default)]
    pub env_overrides: Vec<String>,
}

impl PlatformPaths {
    /// Path templates for the current OS (`macos` / `linux` / `windows` from `platforms.yaml`).
    pub fn templates_for_host(&self) -> Vec<String> {
        #[cfg(target_os = "macos")]
        {
            if let Some(ref paths) = self.macos {
                return paths.clone();
            }
        }
        #[cfg(target_os = "linux")]
        {
            if let Some(ref paths) = self.linux {
                return paths.clone();
            }
        }
        #[cfg(windows)]
        {
            if let Some(ref paths) = self.windows {
                return paths.clone();
            }
        }
        // Dev / unknown target: prefer explicit lists, then cross-platform expand of windows templates.
        self.macos
            .clone()
            .or_else(|| self.linux.clone())
            .or_else(|| self.windows.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlatformRegistry {
    pub platforms: Vec<PlatformEntry>,
}

impl PlatformRegistry {
    pub fn load_embedded() -> Result<Self> {
        const YAML: &str = include_str!("../registry/platforms.yaml");
        serde_yaml::from_str(YAML).context("parse embedded platforms.yaml")
    }

    pub fn load_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("read registry {}", path.display()))?;
        serde_yaml::from_str(&content).context("parse platforms.yaml")
    }

    pub fn get(&self, id: &str) -> Option<&PlatformEntry> {
        self.platforms.iter().find(|p| p.id == id)
    }

    pub fn implemented_ids(&self) -> Vec<&str> {
        self.platforms
            .iter()
            .filter(|p| p.status == "implemented" || p.status == "optional_api")
            .map(|p| p.id.as_str())
            .collect()
    }
}
