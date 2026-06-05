use super::{Adapter, ProbeHit};
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
        let db = hermes_home().join("state.db");
        Ok(vec![ProbeHit {
            path: db.display().to_string(),
            exists: db.exists(),
            size_bytes: std::fs::metadata(&db).ok().map(|m| m.len()),
            note: Some("messages table with input_tokens/output_tokens".into()),
        }])
    }

    fn scan(&self, ingested_at: i64) -> Result<Vec<UsageEvent>> {
        let db_path = hermes_home().join("state.db");
        if !db_path.exists() {
            return Ok(Vec::new());
        }
        scan_hermes_db(&db_path, ingested_at)
    }
}

fn scan_hermes_db(db_path: &PathBuf, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let conn = open_foreign_db(db_path)?;
    let source = db_path.display().to_string();
    let mut events = Vec::new();

    let sql_variants = [
        "SELECT id, session_id, model, input_tokens, output_tokens, created_at FROM messages WHERE input_tokens > 0 OR output_tokens > 0",
        "SELECT rowid, session_id, model, prompt_tokens, completion_tokens, timestamp FROM messages WHERE prompt_tokens > 0 OR completion_tokens > 0",
    ];
    for sql in sql_variants {
        if let Ok(mut stmt) = conn.prepare(sql) {
            let mapped = stmt.query_map([], |row| {
                let id: i64 = row.get(0)?;
                let session_id: String = row.get::<_, Option<String>>(1)?.unwrap_or_else(|| "default".into());
                let model: Option<String> = row.get(2).ok();
                let input: i64 = row.get(3)?;
                let output: i64 = row.get(4)?;
                let ts: Option<i64> = row.get(5).ok();
                Ok((id, session_id, model, input, output, ts))
            });
            if let Ok(rows) = mapped {
                for row in rows.flatten() {
                    let (id, session_id, model, input, output, ts) = row;
                    let mut ev = UsageEvent::new_base(
                        "hermes",
                        PlatformKind::Cli,
                        &session_id,
                        &source,
                        ingested_at,
                    );
                    ev.id = make_event_id("hermes", &format!("{source}:{id}"));
                    ev.call_id = Some(id.to_string());
                    ev.ts = ts.unwrap_or(ingested_at);
                    ev.model = model;
                    ev.input_tokens = input;
                    ev.output_tokens = output;
                    ev.quality = UsageQuality::Exact;
                    ev.compute_total();
                    events.push(ev);
                }
                if !events.is_empty() {
                    return Ok(events);
                }
            }
        }
    }
    Ok(events)
}
