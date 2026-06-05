use super::{Adapter, ProbeHit};
use crate::adapters::cline::parse_ui_messages_public;
use crate::model::{make_event_id, PlatformKind, UsageEvent, UsageQuality};
use crate::paths::{expand_path_template, host, vscode_global_storage};
use crate::util::sqlite_ext::open_foreign_db;
use anyhow::Result;
use std::fs;
use std::path::Path;

pub struct KiloCliAdapter;

impl Adapter for KiloCliAdapter {
    fn id(&self) -> &'static str {
        "kilo_cli"
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        Ok(host::kilo_cli_db_candidates()
            .iter()
            .map(|db| ProbeHit {
                path: db.display().to_string(),
                exists: db.exists(),
                size_bytes: fs::metadata(db).ok().map(|m| m.len()),
                note: Some("kilo.db SQLite".into()),
            })
            .collect())
    }

    fn scan(&self, ingested_at: i64) -> Result<Vec<UsageEvent>> {
        let db = host::kilo_cli_db();
        if !db.exists() {
            return Ok(Vec::new());
        }
        scan_kilo_db(&db, ingested_at)
    }
}

pub struct KiloIdeAdapter;

impl Adapter for KiloIdeAdapter {
    fn id(&self) -> &'static str {
        "kilo_ide"
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        let tasks = vscode_global_storage("kilocode.kilo-code");
        Ok(vec![ProbeHit {
            path: tasks.display().to_string(),
            exists: tasks.exists(),
            size_bytes: None,
            note: Some("Cline-family ui_messages.json".into()),
        }])
    }

    fn scan(&self, ingested_at: i64) -> Result<Vec<UsageEvent>> {
        let mut events = Vec::new();
        for root in [
            vscode_global_storage("kilocode.kilo-code"),
            expand_path_template("%USERPROFILE%\\.kilocode\\cli\\global\\tasks"),
        ] {
            if root.exists() {
                events.extend(scan_kilo_ide_tasks(&root, ingested_at)?);
            }
        }
        Ok(events)
    }
}

fn scan_kilo_ide_tasks(root: &Path, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let mut events = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let task_dir = entry.path();
        if !task_dir.is_dir() {
            continue;
        }
        let task_id = task_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let ui_path = task_dir.join("ui_messages.json");
        if ui_path.exists() {
            let content = fs::read_to_string(&ui_path)?;
            for mut ev in parse_ui_messages_public(
                &content,
                &ui_path,
                task_id,
                "vscode",
                ingested_at,
            ) {
                ev.platform = "kilo_ide".to_string();
                ev.platform_kind = PlatformKind::Ide;
                ev.id = make_event_id("kilo_ide", &format!("{}:{}", ev.source_path, ev.call_id.as_deref().unwrap_or("x")));
                events.push(ev);
            }
        }
    }
    Ok(events)
}

fn scan_kilo_db(db_path: &Path, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let conn = open_foreign_db(db_path)?;
    let source = db_path.display().to_string();
    let mut events = Vec::new();

    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")?
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    for table in tables {
        let sql = format!("SELECT * FROM \"{table}\"");
        if let Ok(mut stmt) = conn.prepare(&sql) {
            let col_count = stmt.column_count();
            if let Ok(rows) = stmt.query_map([], |row| {
                let mut vals = Vec::new();
                for i in 0..col_count {
                    let v: String = row.get(i).unwrap_or_default();
                    vals.push(v);
                }
                Ok(vals)
            }) {
                for (idx, row) in rows.flatten().enumerate() {
                    for cell in row {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cell) {
                            if let Some(mut ev) = usage_from_json(
                                &v,
                                &format!("kilo-{idx}"),
                                &source,
                                ingested_at,
                            ) {
                                events.push(ev);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(events)
}

fn usage_from_json(
    v: &serde_json::Value,
    session_id: &str,
    source: &str,
    ingested_at: i64,
) -> Option<UsageEvent> {
    let usage = v.get("usage").or_else(|| v.get("tokens"))?;
    let input = usage
        .get("input_tokens")
        .or_else(|| usage.get("input"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .or_else(|| usage.get("output"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    if input == 0 && output == 0 {
        return None;
    }
    let id = v.get("id").map(|x| x.to_string()).unwrap_or_default();
    let mut ev = UsageEvent::new_base("kilo_cli", PlatformKind::Cli, session_id, source, ingested_at);
    ev.id = make_event_id("kilo_cli", &format!("{source}:{id}"));
    ev.input_tokens = input;
    ev.output_tokens = output;
    ev.quality = UsageQuality::Exact;
    ev.compute_total();
    Some(ev)
}
