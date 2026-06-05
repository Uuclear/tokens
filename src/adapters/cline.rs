use super::{Adapter, ProbeHit};
use crate::scan_filter::ScanFilter;
use crate::model::{make_event_id, PlatformKind, UsageEvent, UsageQuality};
use crate::paths::{cline_data_dir, vscode_global_storage};
use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::path::Path;

pub struct ClineAdapter;

impl Adapter for ClineAdapter {
    fn id(&self) -> &'static str {
        "cline"
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        Ok(vec![
            ProbeHit {
                path: cline_data_dir().join("tasks").display().to_string(),
                exists: cline_data_dir().join("tasks").exists(),
                size_bytes: None,
                note: Some("CLI surface".into()),
            },
            ProbeHit {
                path: vscode_global_storage("saoudrizwan.claude-dev")
                    .display()
                    .to_string(),
                exists: vscode_global_storage("saoudrizwan.claude-dev").exists(),
                size_bytes: None,
                note: Some("VS Code extension surface".into()),
            },
        ])
    }

    fn scan(&self, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>> {
        let mut events = Vec::new();
        let cli_tasks = cline_data_dir().join("tasks");
        if cli_tasks.exists() {
            events.extend(scan_tasks_dir(&cli_tasks, "cli", ingested_at, filter)?);
        }
        let vscode_tasks = vscode_global_storage("saoudrizwan.claude-dev");
        if vscode_tasks.exists() {
            events.extend(scan_tasks_dir(&vscode_tasks, "vscode", ingested_at, filter)?);
        }
        Ok(events)
    }
}

pub fn scan_tasks_dir_public(
    root: &Path,
    surface: &str,
    ingested_at: i64,
) -> Result<Vec<UsageEvent>> {
    let filter = ScanFilter::parse_all();
    scan_tasks_dir(root, surface, ingested_at, &filter)
}

pub fn parse_ui_messages_public(
    content: &str,
    path: &Path,
    task_id: &str,
    surface: &str,
    ingested_at: i64,
) -> Vec<UsageEvent> {
    parse_ui_messages(content, path, task_id, surface, ingested_at)
}

fn scan_tasks_dir(root: &Path, surface: &str, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>> {
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
            if !filter.should_parse(&ui_path)? {
                continue;
            }
            let content = fs::read_to_string(&ui_path)?;
            events.extend(parse_ui_messages(
                &content,
                &ui_path,
                task_id,
                surface,
                ingested_at,
            ));
        }
    }
    Ok(events)
}

fn parse_ui_messages(
    content: &str,
    path: &Path,
    task_id: &str,
    surface: &str,
    ingested_at: i64,
) -> Vec<UsageEvent> {
    let Ok(messages) = serde_json::from_str::<Value>(content) else {
        return Vec::new();
    };
    let Some(arr) = messages.as_array() else {
        return Vec::new();
    };
    let source = path.display().to_string();
    let mut events = Vec::new();
    for (i, msg) in arr.iter().enumerate() {
        let say = msg.get("say").and_then(|s| s.as_str());
        if say != Some("api_req_started") {
            continue;
        }
        let text = msg.get("text").and_then(|t| t.as_str()).unwrap_or("");
        let Ok(inner) = serde_json::from_str::<Value>(text) else {
            continue;
        };
        let tokens = inner.get("tokens");
        let Some(tokens) = tokens else { continue };
        let input = tokens
            .get("input")
            .or_else(|| tokens.get("prompt"))
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let output = tokens
            .get("output")
            .or_else(|| tokens.get("completion"))
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        if input == 0 && output == 0 {
            continue;
        }
        let mut ev = UsageEvent::new_base("cline", PlatformKind::Hybrid, task_id, &source, ingested_at);
        ev.id = make_event_id("cline", &format!("{source}:{task_id}:{i}"));
        ev.surface = Some(surface.to_string());
        ev.call_id = Some(format!("{i}"));
        ev.input_tokens = input;
        ev.output_tokens = output;
        ev.model = inner
            .get("model")
            .and_then(|m| m.as_str())
            .map(String::from);
        ev.quality = UsageQuality::Exact;
        ev.compute_total();
        events.push(ev);
    }
    events
}
