use super::{Adapter, ProbeHit};
use crate::model::{make_event_id, PlatformKind, UsageEvent, UsageQuality};
use crate::paths::host;
use anyhow::Result;
use serde_json::Value;
use std::fs;
use walkdir::WalkDir;

pub struct ChatboxAdapter;

impl Adapter for ChatboxAdapter {
    fn id(&self) -> &'static str {
        "chatbox"
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        Ok(host::chatbox_roots()
            .iter()
            .map(|p| ProbeHit {
                path: p.display().to_string(),
                exists: p.exists(),
                size_bytes: None,
                note: Some("Local storage + chatbox-blobs; message token fields".into()),
            })
            .collect())
    }

    fn scan(&self, ingested_at: i64) -> Result<Vec<UsageEvent>> {
        let mut events = Vec::new();
        for base in host::chatbox_roots() {
            if base.exists() {
                events.extend(scan_chatbox(&base, ingested_at)?);
            }
        }
        Ok(events)
    }
}

fn scan_chatbox(root: &std::path::Path, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let mut events = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.extension().is_some_and(|x| x == "json")
                || p.file_name().is_some_and(|n| n.to_string_lossy().contains("blob"))
        })
    {
        let path = entry.path();
        if path.is_file() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(v) = serde_json::from_str::<Value>(&content) {
                    events.extend(find_token_fields(&v, path, ingested_at));
                }
            }
        }
    }
    Ok(events)
}

fn find_token_fields(
    v: &Value,
    path: &std::path::Path,
    ingested_at: i64,
) -> Vec<UsageEvent> {
    let mut events = Vec::new();
    let source = path.display().to_string();

    fn walk(v: &Value, source: &str, ingested_at: i64, out: &mut Vec<UsageEvent>) {
        let input = v
            .get("prompt_tokens")
            .or_else(|| v.get("input_tokens"))
            .and_then(|x| x.as_i64());
        let output = v
            .get("completion_tokens")
            .or_else(|| v.get("output_tokens"))
            .and_then(|x| x.as_i64());
        if let (Some(i), Some(o)) = (input, output) {
            if i > 0 || o > 0 {
                let session_id = v
                    .get("id")
                    .or_else(|| v.get("topicId"))
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "chatbox".into());
                let mut ev = UsageEvent::new_base(
                    "chatbox",
                    PlatformKind::Ide,
                    &session_id,
                    source,
                    ingested_at,
                );
                ev.id = make_event_id("chatbox", &format!("{source}:{session_id}:{i}:{o}"));
                ev.input_tokens = i;
                ev.output_tokens = o;
                ev.quality = UsageQuality::Exact;
                ev.compute_total();
                out.push(ev);
            }
        }
        match v {
            Value::Object(map) => {
                for val in map.values() {
                    walk(val, source, ingested_at, out);
                }
            }
            Value::Array(arr) => {
                for val in arr {
                    walk(val, source, ingested_at, out);
                }
            }
            _ => {}
        }
    }

    walk(v, &source, ingested_at, &mut events);
    events
}
