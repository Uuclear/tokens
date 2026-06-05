//! User-configured platform enablement and scan path overrides (from `tokens setup`).

use crate::db::TokenStore;
use crate::paths::expand_path_template;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;

const ENABLED_KEY: &str = "setup.enabled";
const PATH_PREFIX: &str = "paths.";

static INSTALLED: OnceLock<UserPathConfig> = OnceLock::new();

#[derive(Debug, Clone, Default)]
pub struct UserPathConfig {
    /// When set, only these platform ids are scanned.
    pub enabled: Option<HashSet<String>>,
    /// Custom scan roots per platform (`;`-separated in DB).
    pub overrides: HashMap<String, Vec<PathBuf>>,
}

impl UserPathConfig {
    pub fn load(store: &TokenStore) -> Result<Self> {
        let mut cfg = Self::default();
        if let Some(raw) = store.get_config(ENABLED_KEY)? {
            let set: HashSet<String> = raw
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !set.is_empty() {
                cfg.enabled = Some(set);
            }
        }
        for key in store.list_config_keys(Some(PATH_PREFIX))? {
            if let Some(id) = key.strip_prefix(PATH_PREFIX) {
                if let Some(val) = store.get_config(&key)? {
                    let paths = parse_path_list(&val);
                    if !paths.is_empty() {
                        cfg.overrides.insert(id.to_string(), paths);
                    }
                }
            }
        }
        Ok(cfg)
    }

    pub fn save_enabled(&self, store: &TokenStore, ids: &[String]) -> Result<()> {
        store.set_config(ENABLED_KEY, &ids.join(","))?;
        Ok(())
    }

    pub fn save_paths(&self, store: &TokenStore, platform: &str, paths: &[PathBuf]) -> Result<()> {
        let key = format!("{PATH_PREFIX}{platform}");
        if paths.is_empty() {
            store.delete_config(&key)?;
        } else {
            let joined = paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(";");
            store.set_config(&key, &joined)?;
        }
        Ok(())
    }

    pub fn clear_path_overrides(store: &TokenStore) -> Result<()> {
        for key in store.list_config_keys(Some(PATH_PREFIX))? {
            store.delete_config(&key)?;
        }
        Ok(())
    }

    pub fn is_enabled(&self, platform: &str) -> bool {
        match &self.enabled {
            Some(set) => set.contains(platform),
            None => true,
        }
    }

    pub fn override_paths(&self, platform: &str) -> Option<&[PathBuf]> {
        self.overrides
            .get(platform)
            .filter(|v| !v.is_empty())
            .map(|v| v.as_slice())
    }
}

pub fn install(cfg: UserPathConfig) {
    let _ = INSTALLED.set(cfg);
}

static FALLBACK: OnceLock<UserPathConfig> = OnceLock::new();

pub fn get() -> &'static UserPathConfig {
    INSTALLED.get().unwrap_or_else(|| {
        FALLBACK.get_or_init(|| UserPathConfig {
            enabled: None,
            overrides: HashMap::new(),
        })
    })
}

pub fn parse_path_list(raw: &str) -> Vec<PathBuf> {
    raw.split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(expand_path_template)
        .collect()
}

pub fn paths_key(platform: &str) -> String {
    format!("{PATH_PREFIX}{platform}")
}
