use super::{Adapter, ProbeHit};
use crate::scan_filter::ScanFilter;
use crate::model::{make_event_id, PlatformKind, UsageEvent, UsageQuality};
use crate::paths::opencode_data_dirs;
use crate::util::sqlite_ext::{list_tables, open_foreign_db, table_columns};
use anyhow::Result;
use rusqlite::Connection;
use serde_json::Value;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub struct OpenCodeAdapter;

impl Adapter for OpenCodeAdapter {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        let mut hits = Vec::new();
        for base in opencode_data_dirs() {
            let db = base.join("opencode.db");
            let note = if db.exists() {
                open_foreign_db(&db)
                    .ok()
                    .and_then(|c| list_tables(&c).ok())
                    .map(|t| format!("tables: {}", t.join(", ")))
            } else {
                None
            };
            hits.push(ProbeHit {
                path: base.display().to_string(),
                exists: base.exists(),
                size_bytes: fs::metadata(&db).ok().map(|m| m.len()),
                note,
            });
        }
        Ok(hits)
    }

    fn scan(&self, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>> {
        let mut events = Vec::new();
        for base in opencode_data_dirs() {
            if !base.exists() {
                continue;
            }
            for name in ["opencode.db", "opencode-stable.db"] {
                let db_path = base.join(name);
                if db_path.exists() {
                    events.extend(scan_sqlite(&db_path, ingested_at, filter)?);
                }
            }
            events.extend(scan_legacy_json(&base.join("storage/message"), ingested_at, filter)?);
            events.extend(scan_legacy_json(&base.join("storage/part"), ingested_at, filter)?);
        }
        Ok(events)
    }
}

fn scan_sqlite(db_path: &Path, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>> {
    if !filter.should_parse(db_path)? {
        return Ok(Vec::new());
    }
    let conn = open_foreign_db(db_path)?;
    let source = db_path.display().to_string();
    let mut events = Vec::new();

    // Known OpenCode 1.2+ queries
    let fixed_queries = [
        "SELECT id, session_id, provider_id, model_id, tokens, cost, time_created FROM message WHERE tokens IS NOT NULL",
        "SELECT id, sessionID, providerID, modelID, tokens, cost, created_at FROM messages WHERE tokens IS NOT NULL",
        "SELECT id, session_id, provider_id, model_id, tokens, cost, created_at FROM message WHERE tokens IS NOT NULL",
    ];
    for sql in fixed_queries {
        if let Ok(mut stmt) = conn.prepare(sql) {
            if let Ok(rows) = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                let session_id: String = row.get(1)?;
                let provider: Option<String> = row.get(2).ok();
                let model: Option<String> = row.get(3).ok();
                let tokens_json: String = row.get(4)?;
                let cost: Option<f64> = row.get(5).ok();
                let ts: Option<i64> = row.get(6).ok();
                Ok((id, session_id, provider, model, tokens_json, cost, ts))
            }) {
                for row in rows.flatten() {
                    if let Some(ev) = parse_message_row(row, &source, ingested_at) {
                        events.push(ev);
                    }
                }
                if !events.is_empty() {
                    return Ok(events);
                }
            }
        }
    }

    // Schema discovery: scan all tables for JSON token blobs
    for table in list_tables(&conn)? {
        let cols = table_columns(&conn, &table)?;
        events.extend(scan_table_dynamic(&conn, &table, &cols, &source, ingested_at)?);
    }
    Ok(events)
}

fn scan_table_dynamic(
    conn: &Connection,
    table: &str,
    cols: &[String],
    source: &str,
    ingested_at: i64,
) -> Result<Vec<UsageEvent>> {
    let mut events = Vec::new();
    let token_cols: Vec<_> = cols
        .iter()
        .filter(|c| {
            let l = c.to_lowercase();
            l.contains("token") || l == "data" || l == "body" || l == "value"
        })
        .collect();
    if token_cols.is_empty() {
        return Ok(events);
    }
    let col_list = cols
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let col_names: Vec<String> = cols.to_vec();
    let sql = format!("SELECT {col_list} FROM \"{table}\" LIMIT 10000");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let mut vals = Vec::new();
        for (i, name) in col_names.iter().enumerate() {
            let text: String = row.get(i).unwrap_or_default();
            vals.push((name.clone(), text));
        }
        Ok(vals)
    })?;
    for (idx, row) in rows.flatten().enumerate() {
        for (col_name, cell) in row {
            if let Some(ev) = parse_cell_as_usage(&cell, table, &col_name, idx, source, ingested_at) {
                events.push(ev);
            }
        }
    }
    Ok(events)
}

fn parse_cell_as_usage(
    cell: &str,
    table: &str,
    col: &str,
    idx: usize,
    source: &str,
    ingested_at: i64,
) -> Option<UsageEvent> {
    let v: Value = serde_json::from_str(cell).ok()?;
    let tokens = v.get("tokens").or_else(|| v.pointer("/usage"))?;
    let input = tokens
        .get("input")
        .or_else(|| tokens.get("input_tokens"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let output = tokens
        .get("output")
        .or_else(|| tokens.get("output_tokens"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    if input == 0 && output == 0 {
        // nested search one level
        if let Some(obj) = v.as_object() {
            for val in obj.values() {
                if let Some(ev) = parse_cell_as_usage(
                    &val.to_string(),
                    table,
                    col,
                    idx,
                    source,
                    ingested_at,
                ) {
                    return Some(ev);
                }
            }
        }
        return None;
    }
    let session_id = v
        .get("sessionID")
        .or_else(|| v.get("session_id"))
        .and_then(|x| x.as_str())
        .map(String::from)
        .unwrap_or_else(|| format!("{table}-{idx}"));
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .map(String::from)
        .unwrap_or_else(|| format!("{idx}"));
    let mut ev = UsageEvent::new_base("opencode", PlatformKind::Cli, &session_id, source, ingested_at);
    ev.id = make_event_id("opencode", &format!("{source}:{table}:{id}"));
    ev.surface = Some("cli".into());
    ev.call_id = Some(id.to_string());
    ev.provider = v
        .get("providerID")
        .or_else(|| v.get("provider_id"))
        .and_then(|x| x.as_str())
        .map(String::from);
    ev.model = v
        .get("modelID")
        .or_else(|| v.get("model_id"))
        .and_then(|x| x.as_str())
        .map(String::from);
    ev.input_tokens = input;
    ev.output_tokens = output;
    ev.cost_usd = v.get("cost").and_then(|x| x.as_f64());
    ev.quality = UsageQuality::Exact;
    ev.compute_total();
    Some(ev)
}

type MessageRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    Option<f64>,
    Option<i64>,
);

fn parse_message_row(
    row: MessageRow,
    source: &str,
    ingested_at: i64,
) -> Option<UsageEvent> {
    let (id, session_id, provider, model, tokens_json, cost, ts) = row;
    let tokens: Value = serde_json::from_str(&tokens_json).ok()?;
    let input = tokens.get("input").and_then(|x| x.as_i64()).unwrap_or(0);
    let output = tokens.get("output").and_then(|x| x.as_i64()).unwrap_or(0);
    if input == 0 && output == 0 {
        return None;
    }
    let mut ev = UsageEvent::new_base("opencode", PlatformKind::Cli, &session_id, source, ingested_at);
    ev.id = make_event_id("opencode", &format!("{source}:{id}"));
    ev.call_id = Some(id);
    ev.surface = Some("cli".into());
    ev.ts = ts.unwrap_or(ingested_at);
    ev.provider = provider;
    ev.model = model;
    ev.input_tokens = input;
    ev.output_tokens = output;
    ev.cost_usd = cost;
    ev.quality = UsageQuality::Exact;
    ev.compute_total();
    Some(ev)
}

fn scan_legacy_json(messages_dir: &Path, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>> {
    let mut events = Vec::new();
    if !messages_dir.exists() {
        return Ok(events);
    }
    for entry in WalkDir::new(messages_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
    {
        let path = entry.path();
        if !filter.should_parse(path)? {
            continue;
        }
        let content = fs::read_to_string(path)?;
        let Ok(v) = serde_json::from_str::<Value>(&content) else {
            continue;
        };
        let tokens = v.get("tokens");
        let Some(tokens) = tokens else { continue };
        let input = tokens.get("input").and_then(|x| x.as_i64()).unwrap_or(0);
        let output = tokens.get("output").and_then(|x| x.as_i64()).unwrap_or(0);
        if input == 0 && output == 0 {
            continue;
        }
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("unknown");
        let session_id = v
            .get("sessionID")
            .or_else(|| v.get("session_id"))
            .and_then(|x| x.as_str())
            .unwrap_or("unknown");
        let source = path.display().to_string();
        let mut ev =
            UsageEvent::new_base("opencode", PlatformKind::Cli, session_id, &source, ingested_at);
        ev.id = make_event_id("opencode", &format!("{source}:{id}"));
        ev.call_id = Some(id.to_string());
        ev.surface = Some("cli".into());
        ev.provider = v
            .get("providerID")
            .and_then(|x| x.as_str())
            .map(String::from);
        ev.model = v.get("modelID").and_then(|x| x.as_str()).map(String::from);
        ev.input_tokens = input;
        ev.output_tokens = output;
        ev.cost_usd = v.get("cost").and_then(|x| x.as_f64());
        ev.quality = UsageQuality::Exact;
        ev.compute_total();
        events.push(ev);
    }
    Ok(events)
}
