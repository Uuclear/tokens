use super::{Adapter, ProbeHit};
use crate::model::{make_event_id, PlatformKind, UsageEvent, UsageQuality};
use crate::paths::discovery::{cursor_roots, CursorRootKind};
use crate::util::json_extract::extract_model;
use crate::util::sqlite_ext::open_foreign_db;
use anyhow::Result;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub struct CursorAdapter;

impl Adapter for CursorAdapter {
    fn id(&self) -> &'static str {
        "cursor"
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        Ok(cursor_roots()
            .into_iter()
            .map(|r| ProbeHit {
                path: r.path.display().to_string(),
                exists: r.path.exists(),
                size_bytes: std::fs::metadata(&r.path).ok().map(|m| m.len()),
                note: Some(format!("surface={} kind={:?}", r.surface, r.kind)),
            })
            .collect())
    }

    fn scan(&self, ingested_at: i64) -> Result<Vec<UsageEvent>> {
        let mut events = Vec::new();
        for root in cursor_roots() {
            match root.kind {
                CursorRootKind::Vscdb => {
                    events.extend(scan_vscdb(&root.path, &root.surface, ingested_at)?);
                }
                CursorRootKind::AgentTranscripts => {
                    events.extend(scan_agent_transcripts(&root.path, ingested_at)?);
                }
                CursorRootKind::CliStore => {
                    events.extend(scan_cli_store(&root.path, ingested_at)?);
                }
            }
        }
        Ok(events)
    }
}

/// Global / workspace model hints from VS Code `ItemTable`.
fn load_itemtable_default_model(conn: &Connection) -> Option<String> {
    const CANDIDATE_KEYS: &[&str] = &[
        "cursorai.model",
        "cursor.general.model",
        "cursor.chat.defaultModel",
        "aichat.model",
        "workbench.panel.aichat.model",
    ];
    for key in CANDIDATE_KEYS {
        if let Ok(text) = conn.query_row(
            "SELECT value FROM ItemTable WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get::<_, String>(0),
        ) {
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                if let Some(m) = extract_model(&v) {
                    return Some(m);
                }
            }
            let trimmed = text.trim().trim_matches('"');
            if !trimmed.is_empty() && trimmed.len() < 120 && !trimmed.starts_with('{') {
                return Some(trimmed.to_string());
            }
        }
    }
    let mut stmt = conn
        .prepare(
            "SELECT value FROM ItemTable WHERE key LIKE '%defaultModel%' OR key LIKE '%selectedModel%' OR key LIKE '%chat.model%' LIMIT 5",
        )
        .ok()?;
    for row in stmt.query_map([], |r| r.get::<_, String>(0)).ok()?.flatten() {
        if let Ok(v) = serde_json::from_str::<Value>(&row) {
            if let Some(m) = extract_model(&v) {
                return Some(m);
            }
        }
    }
    None
}

fn load_composer_models(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let mut stmt =
        conn.prepare("SELECT key, value FROM cursorDiskKV WHERE key LIKE 'composerData:%'")?;
    let rows = stmt.query_map([], |row| {
        let key: String = row.get(0)?;
        let value: String = row.get(1)?;
        Ok((key, value))
    })?;
    for row in rows.flatten() {
        let (key, text) = row;
        let session_id = key
            .strip_prefix("composerData:")
            .unwrap_or(&key)
            .to_string();
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if let Some(m) = extract_model(&v) {
            map.insert(session_id, m);
        }
    }
    Ok(map)
}

fn scan_vscdb(db_path: &Path, surface: &str, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let conn = open_foreign_db(db_path)?;
    let source = db_path.display().to_string();
    let session_models = load_composer_models(&conn)?;
    let default_model = load_itemtable_default_model(&conn);
    let mut events = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT key, value FROM cursorDiskKV WHERE key LIKE 'bubbleId:%' OR key LIKE 'composerData:%'",
    )?;
    let rows = stmt.query_map([], |row| {
        let key: String = row.get(0)?;
        let value: String = row.get(1)?;
        Ok((key, value))
    })?;

    for row in rows.flatten() {
        let (key, text) = row;
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if key.starts_with("bubbleId:") {
            let session_id = key.split(':').nth(1).unwrap_or("unknown").to_string();
            let session_model = session_models
                .get(&session_id)
                .cloned()
                .or_else(|| default_model.clone());
            if let Some(ev) = parse_bubble(
                &v,
                &key,
                &source,
                surface,
                ingested_at,
                session_model,
            ) {
                events.push(ev);
            }
        } else if key.starts_with("composerData:") {
            if let Some(usage) = v.get("usageData") {
                let session_id = key.strip_prefix("composerData:").unwrap_or(&key);
                let model = session_models
                    .get(session_id)
                    .cloned()
                    .or_else(|| extract_model(&v))
                    .or_else(|| default_model.clone());
                if let Some(ev) = parse_composer_usage(
                    usage,
                    &key,
                    &source,
                    surface,
                    ingested_at,
                    model,
                ) {
                    events.push(ev);
                }
            }
        }
    }
    Ok(events)
}

fn parse_bubble(
    v: &Value,
    key: &str,
    source: &str,
    surface: &str,
    ingested_at: i64,
    session_model: Option<String>,
) -> Option<UsageEvent> {
    let token_count = v.get("tokenCount")?;
    let input = token_count
        .get("inputTokens")
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let output = token_count
        .get("outputTokens")
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let mut quality = UsageQuality::Exact;
    let (input, output) = if input == 0 && output == 0 {
        let text = v.get("text").and_then(|t| t.as_str()).unwrap_or("");
        let est = (text.len() as i64 + 3) / 4;
        quality = UsageQuality::Estimated;
        if v.get("type").and_then(|t| t.as_i64()) == Some(1) {
            (est, 0)
        } else {
            (0, est)
        }
    } else {
        (input, output)
    };
    if input == 0 && output == 0 {
        return None;
    }
    let session_id = key.split(':').nth(1).unwrap_or("unknown");
    let bubble_id = key.split(':').last().unwrap_or("unknown");
    let mut ev = UsageEvent::new_base("cursor", PlatformKind::Ide, session_id, source, ingested_at);
    ev.id = make_event_id("cursor", &format!("{key}:{bubble_id}"));
    ev.surface = Some(surface.to_string());
    ev.call_id = Some(bubble_id.to_string());
    ev.ts = v
        .get("createdAt")
        .and_then(|x| x.as_i64())
        .unwrap_or(ingested_at);
    ev.model = extract_model(v).or(session_model);
    ev.input_tokens = input;
    ev.output_tokens = output;
    ev.quality = quality;
    ev.compute_total();
    Some(ev)
}

fn parse_composer_usage(
    usage: &Value,
    key: &str,
    source: &str,
    surface: &str,
    ingested_at: i64,
    model: Option<String>,
) -> Option<UsageEvent> {
    let default = usage.get("default")?;
    let amount = default.get("amount").and_then(|x| x.as_i64()).unwrap_or(0);
    if amount == 0 {
        return None;
    }
    let session_id = key.strip_prefix("composerData:").unwrap_or(key);
    let mut ev = UsageEvent::new_base("cursor", PlatformKind::Ide, session_id, source, ingested_at);
    ev.id = make_event_id("cursor", &format!("{key}:usage"));
    ev.surface = Some(surface.to_string());
    ev.model = model;
    ev.output_tokens = amount;
    ev.quality = UsageQuality::Estimated;
    ev.compute_total();
    Some(ev)
}

/// Agent mode / Cursor CLI conversation logs under `.cursor/projects/*/agent-transcripts/`.
fn scan_agent_transcripts(projects_root: &Path, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let mut events = Vec::new();
    if !projects_root.exists() {
        return Ok(events);
    }
    for entry in WalkDir::new(projects_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .components()
                .any(|c| c.as_os_str() == "agent-transcripts")
                && e.path().extension().is_some_and(|x| x == "jsonl")
        })
    {
        let path = entry.path();
        let session_id = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .or_else(|| path.file_stem().and_then(|s| s.to_str()))
            .unwrap_or("unknown")
            .to_string();
        let source = path.display().to_string();
        if let Ok(content) = fs::read_to_string(path) {
            events.extend(parse_agent_transcript_jsonl(
                &content,
                &source,
                &session_id,
                ingested_at,
            ));
        }
    }
    Ok(events)
}

fn parse_agent_transcript_jsonl(
    content: &str,
    source: &str,
    session_id: &str,
    ingested_at: i64,
) -> Vec<UsageEvent> {
    let mut events = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let role = v.get("role").and_then(|r| r.as_str()).unwrap_or("");
        if role != "assistant" {
            continue;
        }
        let mut chars = 0i64;
        if let Some(content_arr) = v.pointer("/message/content").and_then(|c| c.as_array()) {
            for part in content_arr {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    chars += text.len() as i64;
                }
            }
        }
        if chars == 0 {
            continue;
        }
        let est_in = 0;
        let est_out = (chars + 3) / 4;
        let mut ev = UsageEvent::new_base("cursor", PlatformKind::Cli, session_id, source, ingested_at);
        ev.id = make_event_id("cursor", &format!("{source}:agent:{i}"));
        ev.surface = Some("agent".into());
        ev.call_id = Some(format!("agent:{i}"));
        ev.ts = ingested_at;
        ev.input_tokens = est_in;
        ev.output_tokens = est_out;
        ev.quality = UsageQuality::Estimated;
        ev.compute_total();
        events.push(ev);
    }
    events
}

/// Optional Cursor CLI state under `%USERPROFILE%\.cursor\` (non-IDE).
fn scan_cli_store(cursor_home: &Path, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let mut events = Vec::new();
    let chats = cursor_home.join("chats");
    if chats.exists() {
        for entry in WalkDir::new(&chats)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl" || x == "json"))
        {
            let path = entry.path();
            if let Ok(content) = fs::read_to_string(path) {
                let session_id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let source = path.display().to_string();
                if path.extension().is_some_and(|x| x == "jsonl") {
                    events.extend(parse_agent_transcript_jsonl(
                        &content,
                        &source,
                        &session_id,
                        ingested_at,
                    ));
                }
            }
        }
    }
    Ok(events)
}
