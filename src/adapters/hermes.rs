use super::{Adapter, ProbeHit};
use crate::scan_filter::ScanFilter;
use crate::model::{make_event_id, PlatformKind, UsageEvent, UsageQuality};
use crate::paths::hermes_home;
use crate::util::sqlite_ext::open_foreign_db;
use anyhow::Result;
use std::path::PathBuf;

pub struct HermesAdapter;

impl Adapter for HermesAdapter {
    fn id(&self) -> &'static str {
        "hermes"
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        let home = hermes_home();
        let db = home.join("state.db");
        Ok(vec![ProbeHit {
            path: db.display().to_string(),
            exists: db.exists(),
            size_bytes: std::fs::metadata(&db).ok().map(|m| m.len()),
            note: Some("sessions table with per-session token totals".into()),
        }])
    }

    fn scan(&self, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>> {
        let db_path = hermes_home().join("state.db");
        if !db_path.exists() {
            return Ok(Vec::new());
        }
        if !filter.should_parse(&db_path)? {
            return Ok(Vec::new());
        }
        scan_hermes_db(&db_path, ingested_at)
    }
}

fn scan_hermes_db(db_path: &PathBuf, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let conn = open_foreign_db(db_path)?;
    let source = db_path.display().to_string();
    let mut events = Vec::new();

    let sql = "SELECT id, model, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, reasoning_tokens, started_at \
               FROM sessions \
               WHERE input_tokens > 0 OR output_tokens > 0 OR cache_read_tokens > 0 OR cache_write_tokens > 0 OR reasoning_tokens > 0";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let model: Option<String> = row.get(1).ok();
        let input: i64 = row.get(2)?;
        let output: i64 = row.get(3)?;
        let cache_read: i64 = row.get(4)?;
        let cache_write: i64 = row.get(5)?;
        let reasoning: i64 = row.get(6)?;
        let ts: Option<f64> = row.get(7).ok();
        Ok((id, model, input, output, cache_read, cache_write, reasoning, ts))
    })?;

    for row in rows.flatten() {
        let (id, model, input, output, cache_read, cache_write, reasoning, ts) = row;
        let mut ev = UsageEvent::new_base(
            "hermes",
            PlatformKind::Cli,
            &id,
            &source,
            ingested_at,
        );
        ev.id = make_event_id("hermes", &format!("{source}:{id}"));
        ev.call_id = Some(id.clone());
        ev.surface = Some("cli".into());
        ev.ts = ts
            .map(|t| (t * 1000.0) as i64)
            .unwrap_or(ingested_at);
        ev.model = model;
        ev.input_tokens = input;
        ev.output_tokens = output;
        ev.cache_read_tokens = cache_read;
        ev.cache_write_tokens = cache_write;
        ev.reasoning_tokens = reasoning;
        ev.quality = UsageQuality::Exact;
        ev.compute_total();
        events.push(ev);
    }

    Ok(events)
}
