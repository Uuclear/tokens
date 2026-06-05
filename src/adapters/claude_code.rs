use super::{Adapter, ProbeHit};
use crate::model::{make_event_id, PlatformKind, UsageEvent, UsageQuality};
use crate::paths::discovery::{claude_all_roots, ClaudeRoot};
use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub struct ClaudeCodeAdapter;

impl Adapter for ClaudeCodeAdapter {
    fn id(&self) -> &'static str {
        "claude_code"
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        let mut hits = Vec::new();
        for root in claude_all_roots() {
            hits.push(ProbeHit {
                path: root.path.display().to_string(),
                exists: root.path.exists(),
                size_bytes: None,
                note: Some(format!("surface={} ({})", root.surface, root.label)),
            });
        }
        Ok(hits)
    }

    fn scan(&self, ingested_at: i64) -> Result<Vec<UsageEvent>> {
        let mut events = Vec::new();
        for root in claude_all_roots() {
            if !root.path.exists() {
                continue;
            }
            for entry in WalkDir::new(&root.path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
            {
                let path = entry.path();
                if let Ok(content) = fs::read_to_string(path) {
                    let file_session = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let project_path = path
                        .parent()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .map(|s| s.replace('-', std::path::MAIN_SEPARATOR_STR));
                    events.extend(parse_jsonl(
                        &content,
                        path,
                        &file_session,
                        &root,
                        project_path,
                        ingested_at,
                    ));
                }
            }
        }
        Ok(events)
    }
}

fn parse_jsonl(
    content: &str,
    path: &Path,
    file_session: &str, // file stem fallback
    root: &ClaudeRoot,
    project_path: Option<String>,
    ingested_at: i64,
) -> Vec<UsageEvent> {
    let mut events = Vec::new();
    let source = path.display().to_string();
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let is_assistant = v.get("type").and_then(|t| t.as_str()) == Some("assistant")
            || v.pointer("/message/role").and_then(|r| r.as_str()) == Some("assistant");
        if !is_assistant {
            continue;
        }
        let usage = v
            .pointer("/message/usage")
            .or_else(|| v.get("usage"));
        let Some(usage) = usage else { continue };
        let input = usage
            .get("input_tokens")
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let output = usage
            .get("output_tokens")
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let cache_read = usage
            .get("cache_read_input_tokens")
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let cache_write = usage
            .get("cache_creation_input_tokens")
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        if input == 0 && output == 0 && cache_read == 0 && cache_write == 0 {
            continue;
        }
        let session_id = v
            .get("sessionId")
            .and_then(|x| x.as_str())
            .map(String::from)
            .unwrap_or_else(|| file_session.to_string());
        let surface = claude_surface(root, &v);
        let call_id = v
            .get("requestId")
            .or_else(|| v.pointer("/message/id"))
            .or_else(|| v.get("uuid"))
            .and_then(|x| x.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("{session_id}:{i}"));
        let ts = v
            .get("timestamp")
            .and_then(parse_ts)
            .unwrap_or(ingested_at);
        let model = v
            .pointer("/message/model")
            .or_else(|| v.get("model"))
            .and_then(|x| x.as_str())
            .map(String::from);
        let mut ev = UsageEvent::new_base("claude_code", PlatformKind::Cli, &session_id, &source, ingested_at);
        ev.id = make_event_id("claude_code", &format!("{source}:{call_id}"));
        ev.call_id = Some(call_id);
        ev.surface = Some(surface);
        ev.ts = ts;
        ev.project_path = project_path.clone().or_else(|| {
            v.get("cwd").and_then(|c| c.as_str()).map(String::from)
        });
        ev.model = model;
        ev.input_tokens = input;
        ev.output_tokens = output;
        ev.cache_read_tokens = cache_read;
        ev.cache_write_tokens = cache_write;
        ev.quality = UsageQuality::Exact;
        ev.compute_total();
        events.push(ev);
    }
    events
}

fn claude_surface(root: &ClaudeRoot, v: &Value) -> String {
    if root.surface == "desktop" {
        return "desktop".into();
    }
    match v.get("entrypoint").and_then(|x| x.as_str()) {
        Some(ep) if ep.contains("desktop") || ep.starts_with("claude-desktop") => "desktop".into(),
        Some("cli") | Some("") => "cli".into(),
        Some(ep) => ep.to_string(),
        None => root.surface.clone(),
    }
}

fn parse_ts(v: &Value) -> Option<i64> {
    if let Some(n) = v.as_i64() {
        return Some(if n < 10_000_000_000 { n * 1000 } else { n });
    }
    v.as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp_millis())
}
