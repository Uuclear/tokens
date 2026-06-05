//! Skip unchanged source files before expensive adapter parsing.

use crate::db::TokenStore;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct ScanFilter {
    full_rescan: bool,
    fingerprints: HashMap<String, (i64, i64)>,
}

impl ScanFilter {
    pub fn load(store: &TokenStore, platform: &str, full_rescan: bool) -> Result<Self> {
        if full_rescan {
            return Ok(Self {
                full_rescan: true,
                fingerprints: HashMap::new(),
            });
        }
        Ok(Self {
            full_rescan: false,
            fingerprints: store.load_fingerprints(platform)?,
        })
    }

    pub fn should_parse(&self, path: &Path) -> Result<bool> {
        if self.full_rescan {
            return Ok(true);
        }
        if !path.exists() {
            return Ok(false);
        }
        let meta = fs::metadata(path)?;
        let mtime = file_mtime_secs(&meta);
        let size = meta.len() as i64;
        let source = path.display().to_string();
        Ok(match self.fingerprints.get(&source) {
            Some(&(m, s)) => m != mtime || s != size,
            None => true,
        })
    }

    pub fn parse_all() -> Self {
        Self {
            full_rescan: true,
            fingerprints: HashMap::new(),
        }
    }
}

pub fn file_mtime_secs(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn source_mtime_size(path: &Path) -> Result<(i64, i64)> {
    let meta = fs::metadata(path)?;
    Ok((file_mtime_secs(&meta), meta.len() as i64))
}
