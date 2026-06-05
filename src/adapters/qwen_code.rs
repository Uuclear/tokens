use super::{Adapter, ProbeHit};
use crate::model::{make_event_id, PlatformKind, UsageEvent, UsageQuality};
use crate::paths::qwen_projects_dir;
use anyhow::Result;
use serde_json::Value;
use std::fs;
use walkdir::WalkDir;

pub struct QwenCodeAdapter;

impl Adapter for QwenCodeAdapter {
    fn id(&self) -> &'static str {
        "qwen_code"
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        let dir = qwen_projects_dir();
        Ok(vec![ProbeHit {
            path: dir.display().to_string(),
            exists: dir.exists(),
            size_bytes: None,
            note: Some("~/.qwen/projects session files".into()),
        }])
    }

    fn scan(&self, ingested_at: i64) -> Result<Vec<UsageEvent>> {
        let root = qwen_projects_dir();
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut events = Vec::new();
        for entry in WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let p = e.path();
                p.extension().is_some_and(|x| x == "json" || x == "jsonl")
            })
        {
            let path = entry.path();
            let content = fs::read_to_string(path)?;
            if path.extension().is_some_and(|x| x == "jsonl") {
                for (i, line) in content.lines().enumerate() {
                    if let Ok(v) = serde_json::from_str::<Value>(line) {
                        if let Some(ev) = extract_usage(&v, path, &format!("line-{i}"), ingested_at)
                        {
                            events.push(ev);
                        }
                    }
                }
            } else if let Ok(v) = serde_json::from_str::<Value>(&content) {
                if let Some(ev) = extract_usage(&v, path, "file", ingested_at) {
                    events.push(ev);
                }
                // Nested stats from CLI output format
                if let Some(usage) = v.pointer("/usage") {
                    if let Some(ev) = extract_usage_from_usage_obj(
                        usage,
                        path,
                        v.get("sessionId").and_then(|s| s.as_str()).unwrap_or("file"),
                        ingested_at,
                    ) {
                        events.push(ev);
                    }
                }
            }
        }
        Ok(events)
    }
}

fn extract_usage(
    v: &Value,
    path: &std::path::Path,
    suffix: &str,
    ingested_at: i64,
) -> Option<UsageEvent> {
    let usage = v
        .get("usage")
        .or_else(|| v.pointer("/stats/usage"))
        .or_else(|| v.get("tokenUsage"))?;
    extract_usage_from_usage_obj(
        usage,
        path,
        v.get("sessionId")
            .and_then(|s| s.as_str())
            .unwrap_or(suffix),
        ingested_at,
    )
}

fn extract_usage_from_usage_obj(
    usage: &Value,
    path: &std::path::Path,
    session_id: &str,
    ingested_at: i64,
) -> Option<UsageEvent> {
    let input = usage
        .get("inputTokens")
        .or_else(|| usage.get("input_tokens"))
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let output = usage
        .get("outputTokens")
        .or_else(|| usage.get("output_tokens"))
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    if input == 0 && output == 0 {
        return None;
    }
    let source = path.display().to_string();
    let mut ev =
        UsageEvent::new_base("qwen_code", PlatformKind::Cli, session_id, &source, ingested_at);
    ev.id = make_event_id("qwen_code", &format!("{source}:{session_id}:{input}:{output}"));
    ev.input_tokens = input;
    ev.output_tokens = output;
    ev.model = usage
        .get("model")
        .and_then(|m| m.as_str())
        .map(String::from);
    ev.quality = UsageQuality::Exact;
    ev.compute_total();
    Some(ev)
}
