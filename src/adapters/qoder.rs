use super::{Adapter, ProbeHit};
use crate::scan_filter::ScanFilter;
use crate::paths::host::{self, app_data_local_dir, user_home};
use anyhow::Result;

pub struct QoderAdapter {
    id: &'static str,
}

impl QoderAdapter {
    pub fn new(id: &'static str) -> Self {
        Self { id }
    }
}

impl Adapter for QoderAdapter {
    fn id(&self) -> &'static str {
        self.id
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        let paths = [
            user_home().join(".lingma"),
            app_data_local_dir().join(".lingma"),
            host::xdg_config_home().join(".lingma"),
        ];
        Ok(paths
            .iter()
            .map(|p| ProbeHit {
                path: p.display().to_string(),
                exists: p.exists(),
                size_bytes: None,
                note: Some(
                    "Research: no stable public token log; config/index only".into(),
                ),
            })
            .collect())
    }

    fn scan(&self, _ingested_at: i64, _filter: &ScanFilter) -> Result<Vec<crate::model::UsageEvent>> {
        // Do not fabricate token data per plan
        Ok(Vec::new())
    }
}
