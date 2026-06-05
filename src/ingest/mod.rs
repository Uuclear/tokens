use crate::adapters::{self, Adapter};
use crate::db::TokenStore;
use crate::scan_filter::{ScanFilter, source_mtime_size};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

pub struct ScanOptions {
    pub only_platform: Option<String>,
    pub full_rescan: bool,
    pub include_optional_api: bool,
}

pub struct ScanResult {
    pub files_scanned: i64,
    pub events_inserted: i64,
    pub events_skipped: i64,
    pub per_platform: Vec<(String, i64, i64)>,
}

pub fn run_scan(store: &TokenStore, options: &ScanOptions) -> Result<ScanResult> {
    let ingested_at = chrono::Utc::now().timestamp_millis();
    let run_id = store.start_ingest_run(options.only_platform.as_deref())?;

    let mut files_scanned = 0i64;
    let mut events_inserted = 0i64;
    let mut events_skipped = 0i64;
    let mut per_platform = Vec::new();

    let adapters: Vec<Box<dyn Adapter>> = if let Some(ref id) = options.only_platform {
        adapters::adapter_by_id(id)
            .map(|a| vec![a])
            .unwrap_or_default()
    } else {
        adapters::all_adapters()
    };

    for adapter in adapters {
        let platform = adapter.id().to_string();
        if !crate::paths::user_config::get().is_enabled(&platform) {
            continue;
        }
        if options.full_rescan {
            let _ = store.delete_platform_events(&platform)?;
        }
        let filter = ScanFilter::load(store, &platform, options.full_rescan)?;
        let events = adapter.scan(ingested_at, &filter)?;
        let mut plat_inserted = 0i64;
        let mut plat_skipped = 0i64;

        let mut by_source: HashMap<String, Vec<crate::model::UsageEvent>> = HashMap::new();
        for event in events {
            by_source
                .entry(event.source_path.clone())
                .or_default()
                .push(event);
        }

        for (source, batch) in by_source {
            files_scanned += 1;
            let (inserted, skipped) = store.insert_events_batch(&batch)?;
            events_inserted += inserted;
            events_skipped += skipped;
            plat_inserted += inserted;
            plat_skipped += skipped;
            if inserted > 0 || options.full_rescan {
                let path = Path::new(&source);
                if path.exists() {
                    let (mtime, size) = source_mtime_size(path)?;
                    store.upsert_fingerprint(&platform, &source, mtime, size, ingested_at)?;
                }
            }
        }
        per_platform.push((platform, plat_inserted, plat_skipped));
    }

    if options.include_optional_api && options.only_platform.is_none() {
        let optional_events = adapters::optional_adapters(store)?;
        for event in optional_events {
            if store.insert_event(&event)? {
                events_inserted += 1;
            } else {
                events_skipped += 1;
            }
        }
    }

    store.finish_ingest_run(run_id, files_scanned, events_inserted, events_skipped, None)?;
    Ok(ScanResult {
        files_scanned,
        events_inserted,
        events_skipped,
        per_platform,
    })
}

