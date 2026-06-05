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

/// Tables that mirror message token data and must not be scanned again.
const SKIP_DYNAMIC_TABLES: &[&str] = &[
    "part",
    "__drizzle_migrations",
    "migration",
    "data_migration",
    "sqlite_sequence",
];

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
                path: db.display().to_string(),
                exists: db.exists(),
                size_bytes: fs::metadata(&db).ok().map(|m| m.len()),
                note: if db.exists() {
                    note
                } else {
                    Some(format!(
                        "no opencode.db in {} (install-only dir?)",
                        base.display()
                    ))
                },
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

    // OpenCode 1.2+: message.data JSON with nested tokens.cache
    if let Ok(events) = scan_message_data_table(&conn, &source, ingested_at) {
        if !events.is_empty() {
            return Ok(events);
        }
    }

    // Older schemas with a dedicated tokens column
    let fixed_queries = [
        "SELECT id, session_id, provider_id, model_id, tokens, cost, time_created FROM message WHERE tokens IS NOT NULL",
        "SELECT id, sessionID, providerID, modelID, tokens, cost, created_at FROM messages WHERE tokens IS NOT NULL",
        "SELECT id, session_id, provider_id, model_id, tokens, cost, created_at FROM message WHERE tokens IS NOT NULL",
    ];
    for sql in fixed_queries {
        if let Ok(mut stmt) = conn.prepare(sql) {
            let mut events = Vec::new();
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

    // Last resort: schema discovery (skip mirrored part rows)
    let mut events = Vec::new();
    for table in list_tables(&conn)? {
        if SKIP_DYNAMIC_TABLES.contains(&table.as_str()) {
            continue;
        }
        let cols = table_columns(&conn, &table)?;
        events.extend(scan_table_dynamic(&conn, &table, &cols, &source, ingested_at)?);
    }
    Ok(events)
}

fn scan_message_data_table(
    conn: &Connection,
    source: &str,
    ingested_at: i64,
) -> Result<Vec<UsageEvent>> {
    let sql = "SELECT id, session_id, data, time_created FROM message";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let session_id: String = row.get(1)?;
        let data: String = row.get(2)?;
        let ts: Option<i64> = row.get(3).ok();
        Ok((id, session_id, data, ts))
    })?;

    let mut events = Vec::new();
    for row in rows.flatten() {
        let (id, session_id, data, ts) = row;
        let Ok(v) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        if let Some(ev) = parse_opencode_payload(&v, &id, &session_id, ts, source, ingested_at) {
            events.push(ev);
        }
    }
    Ok(events)
}

fn parse_opencode_tokens(tokens: &Value) -> Option<(i64, i64, i64, i64, i64)> {
    let input = tokens.get("input").and_then(|x| x.as_i64()).unwrap_or(0);
    let output = tokens.get("output").and_then(|x| x.as_i64()).unwrap_or(0);
    let reasoning = tokens.get("reasoning").and_then(|x| x.as_i64()).unwrap_or(0);
    let cache = tokens.get("cache").and_then(|x| x.as_object());
    let cache_read = cache
        .and_then(|c| c.get("read"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let cache_write = cache
        .and_then(|c| c.get("write"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    if input == 0 && output == 0 && cache_read == 0 && cache_write == 0 && reasoning == 0 {
        return None;
    }
    Some((input, output, cache_read, cache_write, reasoning))
}

fn parse_opencode_payload(
    v: &Value,
    id: &str,
    session_id: &str,
    ts: Option<i64>,
    source: &str,
    ingested_at: i64,
) -> Option<UsageEvent> {
    let tokens = v.get("tokens")?;
    let (input, output, cache_read, cache_write, reasoning) = parse_opencode_tokens(tokens)?;
    let mut ev = UsageEvent::new_base("opencode", PlatformKind::Cli, session_id, source, ingested_at);
    ev.id = make_event_id("opencode", &format!("{source}:{id}"));
    ev.call_id = Some(id.to_string());
    ev.surface = Some("cli".into());
    ev.ts = ts.unwrap_or(ingested_at);
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
    ev.cache_read_tokens = cache_read;
    ev.cache_write_tokens = cache_write;
    ev.reasoning_tokens = reasoning;
    ev.cost_usd = v.get("cost").and_then(|x| x.as_f64());
    ev.quality = UsageQuality::Exact;
    ev.compute_total();
    Some(ev)
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
    let sql = format!("SELECT {col_list} FROM \"{table}\"");
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
    if let Some(tokens) = v.get("tokens") {
        if let Some((input, output, cache_read, cache_write, reasoning)) =
            parse_opencode_tokens(tokens)
        {
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
            let mut ev =
                UsageEvent::new_base("opencode", PlatformKind::Cli, &session_id, source, ingested_at);
            ev.id = make_event_id("opencode", &format!("{source}:{table}:{id}"));
            ev.call_id = Some(id);
            ev.surface = Some("cli".into());
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
            ev.cache_read_tokens = cache_read;
            ev.cache_write_tokens = cache_write;
            ev.reasoning_tokens = reasoning;
            ev.cost_usd = v.get("cost").and_then(|x| x.as_f64());
            ev.quality = UsageQuality::Exact;
            ev.compute_total();
            return Some(ev);
        }
    }
    None
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
    let (input, output, cache_read, cache_write, reasoning) = parse_opencode_tokens(&tokens)?;
    let mut ev = UsageEvent::new_base("opencode", PlatformKind::Cli, &session_id, source, ingested_at);
    ev.id = make_event_id("opencode", &format!("{source}:{id}"));
    ev.call_id = Some(id);
    ev.surface = Some("cli".into());
    ev.ts = ts.unwrap_or(ingested_at);
    ev.provider = provider;
    ev.model = model;
    ev.input_tokens = input;
    ev.output_tokens = output;
    ev.cache_read_tokens = cache_read;
    ev.cache_write_tokens = cache_write;
    ev.reasoning_tokens = reasoning;
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
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("unknown");
        let session_id = v
            .get("sessionID")
            .or_else(|| v.get("session_id"))
            .and_then(|x| x.as_str())
            .unwrap_or("unknown");
        let source = path.display().to_string();
        if let Some(mut ev) = parse_opencode_payload(&v, id, session_id, None, &source, ingested_at) {
            ev.id = make_event_id("opencode", &format!("{source}:{id}"));
            events.push(ev);
        }
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tokens_with_cache() {
        let v: Value = serde_json::from_str(
            r#"{"tokens":{"input":4739,"output":96,"reasoning":0,"cache":{"write":0,"read":8448}}}"#,
        )
        .unwrap();
        let (i, o, cr, cw, r) = parse_opencode_tokens(&v["tokens"]).unwrap();
        assert_eq!(i, 4739);
        assert_eq!(o, 96);
        assert_eq!(cr, 8448);
        assert_eq!(cw, 0);
        assert_eq!(r, 0);
    }
}
