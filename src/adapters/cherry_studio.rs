use super::{Adapter, ProbeHit};
use crate::scan_filter::ScanFilter;
use crate::model::{make_event_id, PlatformKind, UsageEvent, UsageQuality};
use crate::paths::host;
use anyhow::Result;
use serde_json::Value;
use std::fs;
use walkdir::WalkDir;

pub struct CherryStudioAdapter;

impl Adapter for CherryStudioAdapter {
    fn id(&self) -> &'static str {
        "cherry_studio"
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        Ok(host::cherry_studio_roots()
            .iter()
            .map(|p| ProbeHit {
                path: p.display().to_string(),
                exists: p.exists(),
                size_bytes: None,
                note: Some("Electron userData + trace spans".into()),
            })
            .collect())
    }

    fn scan(&self, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>> {
        let mut events = Vec::new();
        for base in host::cherry_studio_roots() {
            if base.exists() {
                events.extend(scan_dir(&base, ingested_at, filter)?);
            }
        }
        Ok(events)
    }
}

fn scan_dir(root: &std::path::Path, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>> {
    let mut events = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.extension().is_some_and(|x| x == "json")
                || p.to_string_lossy().contains("span")
                || p.to_string_lossy().contains("trace")
        })
    {
        let path = entry.path();
        if !filter.should_parse(path)? {
            continue;
        }
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(v) = serde_json::from_str::<Value>(&content) {
                events.extend(extract_from_value(&v, path, ingested_at));
            } else if content.contains("input_tokens") || content.contains("prompt_tokens") {
                for (i, line) in content.lines().enumerate() {
                    if let Ok(v) = serde_json::from_str::<Value>(line) {
                        events.extend(extract_from_value(&v, path, ingested_at));
                    }
                }
            }
        }
    }
    Ok(events)
}

fn extract_from_value(
    v: &Value,
    path: &std::path::Path,
    ingested_at: i64,
) -> Vec<UsageEvent> {
    let mut events = Vec::new();
    let source = path.display().to_string();

    fn walk(v: &Value, source: &str, ingested_at: i64, out: &mut Vec<UsageEvent>) {
        if let Some(input) = v.get("input_tokens").and_then(|x| x.as_i64()) {
            let output = v.get("output_tokens").and_then(|x| x.as_i64()).unwrap_or(0);
            if input > 0 || output > 0 {
                let session_id = v
                    .get("topicId")
                    .or_else(|| v.get("spanId"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("cherry");
                let mut ev = UsageEvent::new_base(
                    "cherry_studio",
                    PlatformKind::Ide,
                    session_id,
                    source,
                    ingested_at,
                );
                ev.id = make_event_id(
                    "cherry_studio",
                    &format!("{source}:{session_id}:{input}:{output}"),
                );
                ev.input_tokens = input;
                ev.output_tokens = output;
                ev.provider = v
                    .get("provider")
                    .and_then(|x| x.as_str())
                    .map(String::from);
                ev.model = v.get("model").and_then(|x| x.as_str()).map(String::from);
                ev.quality = UsageQuality::Exact;
                ev.compute_total();
                out.push(ev);
            }
        }
        if let Some(obj) = v.as_object() {
            for val in obj.values() {
                walk(val, source, ingested_at, out);
            }
        } else if let Some(arr) = v.as_array() {
            for val in arr {
                walk(val, source, ingested_at, out);
            }
        }
    }

    walk(v, &source, ingested_at, &mut events);
    events
}
