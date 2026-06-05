use crate::adapters::{self, Adapter};
use crate::db::TokenStore;
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
        let events = adapter.scan(ingested_at)?;
        let mut plat_inserted = 0i64;
        let mut plat_skipped = 0i64;

        // Fingerprint at source-file granularity (not per event).
        let mut by_source: HashMap<String, Vec<crate::model::UsageEvent>> = HashMap::new();
        for event in events {
            by_source
                .entry(event.source_path.clone())
                .or_default()
                .push(event);
        }

        for (source, batch) in by_source {
            files_scanned += batch.len() as i64;
            if should_skip_file(store, &platform, &source, options.full_rescan)? {
                events_skipped += batch.len() as i64;
                plat_skipped += batch.len() as i64;
                continue;
            }
            let mut any_inserted = false;
            for event in batch {
                if store.insert_event(&event)? {
                    events_inserted += 1;
                    plat_inserted += 1;
                    any_inserted = true;
                } else {
                    events_skipped += 1;
                    plat_skipped += 1;
                }
            }
            if any_inserted || options.full_rescan {
                update_fingerprint(store, &platform, &source, ingested_at)?;
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

fn should_skip_file(
    store: &TokenStore,
    platform: &str,
    source: &str,
    full_rescan: bool,
) -> Result<bool> {
    if full_rescan {
        return Ok(false);
    }
    let path = Path::new(source);
    if !path.exists() {
        return Ok(false);
    }
    let meta = std::fs::metadata(path)?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let size = meta.len() as i64;
    store.should_skip_source(platform, source, mtime, size, full_rescan)
}

fn update_fingerprint(
    store: &TokenStore,
    platform: &str,
    source: &str,
    ingested_at: i64,
) -> Result<()> {
    let path = Path::new(source);
    if !path.exists() {
        return Ok(());
    }
    let meta = std::fs::metadata(path)?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let size = meta.len() as i64;
    store.upsert_fingerprint(platform, source, mtime, size, ingested_at)
}
