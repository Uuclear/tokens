use super::{Adapter, ProbeHit};
use crate::scan_filter::ScanFilter;
use crate::model::{make_event_id, PlatformKind, UsageEvent, UsageQuality};
use crate::paths::codex_home;
use crate::util::json_extract::extract_model;
use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub struct CodexAdapter;

impl Adapter for CodexAdapter {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        let home = codex_home();
        let mut hits = Vec::new();
        for sub in ["sessions", "archived_sessions"] {
            let p = home.join(sub);
            hits.push(ProbeHit {
                path: p.display().to_string(),
                exists: p.exists(),
                size_bytes: None,
                note: Some("rollout-*.jsonl (surface=cli|desktop|ide)".into()),
            });
        }
        Ok(hits)
    }

    fn scan(&self, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>> {
        let mut events = Vec::new();
        let home = codex_home();
        for sub in ["sessions", "archived_sessions"] {
            let root = home.join(sub);
            if !root.exists() {
                continue;
            }
            for entry in WalkDir::new(&root)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
            {
                let path = entry.path();
                if !filter.should_parse(path)? {
                    continue;
                }
                if let Ok(content) = fs::read_to_string(path) {
                    let session_id = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    events.extend(parse_rollout(
                        &content,
                        path,
                        &session_id,
                        ingested_at,
                    ));
                }
            }
        }
        Ok(events)
    }
}

fn parse_rollout(
    content: &str,
    path: &Path,
    session_id: &str,
    ingested_at: i64,
) -> Vec<UsageEvent> {
    let mut events = Vec::new();
    let source = path.display().to_string();
    let mut model: Option<String> = None;
    let mut surface = "cli".to_string();
    for (i, line) in content.lines().enumerate() {
        let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if ty == "session_meta" {
            if let Some(m) = extract_model(&v).or_else(|| {
                v.pointer("/payload/model_provider")
                    .and_then(|x| x.as_str())
                    .map(String::from)
            }) {
                model = Some(m);
            }
            if let Some(origin) = v.pointer("/payload/originator").and_then(|x| x.as_str()) {
                if origin.contains("Desktop") {
                    surface = "desktop".into();
                }
            }
            if let Some(src) = v.pointer("/payload/source").and_then(|x| x.as_str()) {
                if src == "vscode" {
                    surface = "ide".into();
                }
            }
            continue;
        }

        if ty == "turn_context" {
            if let Some(m) = extract_model(&v) {
                model = Some(m);
            }
            continue;
        }

        if ty == "task_started" {
            if let Some(m) = v
                .get("model")
                .and_then(|m| m.as_str())
                .map(String::from)
                .or_else(|| extract_model(&v))
            {
                model = Some(m);
            }
        }

        let usage = if ty == "token_usage" {
            v.get("usage").or(Some(&v))
        } else if ty == "event_msg" {
            let payload_type = v.pointer("/payload/type").and_then(|t| t.as_str());
            if payload_type == Some("task_started") {
                if let Some(m) = extract_model(&v) {
                    model = Some(m);
                }
            }
            if payload_type == Some("token_count") {
                v.pointer("/payload/info/last_token_usage")
                    .or_else(|| v.pointer("/payload/info/total_token_usage"))
            } else {
                v.pointer("/payload/token_count")
                    .or_else(|| v.pointer("/payload/usage"))
            }
        } else {
            v.get("token_count").map(|_| &v)
        };
        let Some(u) = usage else { continue };
        let input = u
            .get("input_tokens")
            .or_else(|| u.get("prompt_tokens"))
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let output = u
            .get("output_tokens")
            .or_else(|| u.get("completion_tokens"))
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let cached = u
            .get("cached_input_tokens")
            .or_else(|| u.get("cache_read_input_tokens"))
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let reasoning = u
            .get("reasoning_output_tokens")
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        if input == 0 && output == 0 && cached == 0 {
            continue;
        }
        let ts = v
            .get("ts")
            .or_else(|| v.get("timestamp"))
            .and_then(|x| {
                x.as_i64()
                    .or_else(|| x.as_f64().map(|f| f as i64))
                    .or_else(|| {
                        x.as_str()
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                            .map(|dt| dt.timestamp_millis())
                    })
            })
            .unwrap_or(ingested_at);
        let call_id = format!("{session_id}:{i}");
        let mut ev =
            UsageEvent::new_base("codex", PlatformKind::Cli, session_id, &source, ingested_at);
        ev.surface = Some(surface.clone());
        ev.id = make_event_id("codex", &format!("{source}:{call_id}"));
        ev.call_id = Some(call_id);
        ev.ts = if ts < 10_000_000_000 { ts * 1000 } else { ts };
        ev.model = model.clone();
        ev.input_tokens = input;
        ev.output_tokens = output;
        ev.cache_read_tokens = cached;
        ev.reasoning_tokens = reasoning;
        ev.quality = UsageQuality::Exact;
        ev.compute_total();
        events.push(ev);
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_event_msg_token_count() {
        let content = include_str!("../../tests/codex_fixture.jsonl");
        let events = parse_rollout(content, Path::new("/tmp/r.jsonl"), "sess", 0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].input_tokens, 100);
        assert_eq!(events[0].output_tokens, 10);
        assert_eq!(events[0].cache_read_tokens, 50);
    }

    #[test]
    fn parses_turn_context_model() {
        let line = r#"{"type":"turn_context","payload":{"model":"gpt-5.5"}}"#;
        let line2 = r#"{"type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10,"output_tokens":2,"cached_input_tokens":0}}}}"#;
        let events = parse_rollout(&format!("{line}\n{line2}"), Path::new("/t.jsonl"), "s", 0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].model.as_deref(), Some("gpt-5.5"));
    }
}
