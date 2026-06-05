use super::{Adapter, ProbeHit};
use crate::scan_filter::ScanFilter;
use crate::model::{make_event_id, PlatformKind, UsageEvent, UsageQuality};
use crate::paths::pi_sessions_dir;
use anyhow::Result;
use serde_json::Value;
use std::fs;
use walkdir::WalkDir;

pub struct PiAdapter;

impl Adapter for PiAdapter {
    fn id(&self) -> &'static str {
        "pi"
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        let dir = pi_sessions_dir();
        Ok(vec![ProbeHit {
            path: dir.display().to_string(),
            exists: dir.exists(),
            size_bytes: None,
            note: Some("~/.pi/agent/sessions — message.usage input/output/cache".into()),
        }])
    }

    fn scan(&self, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>> {
        let root = pi_sessions_dir();
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut events = Vec::new();
        for entry in WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        {
            let path = entry.path();
            if !filter.should_parse(path)? {
                continue;
            }
            let session_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let content = fs::read_to_string(path)?;
            events.extend(parse_pi_jsonl(
                &content,
                path,
                &session_id,
                ingested_at,
            ));
        }
        Ok(events)
    }
}

fn parse_pi_jsonl(
    content: &str,
    path: &std::path::Path,
    file_session: &str,
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
        if v.get("type").and_then(|t| t.as_str()) != Some("message") {
            continue;
        }
        let Some(msg) = v.get("message") else {
            continue;
        };
        let Some(usage) = msg.get("usage") else {
            continue;
        };
        let input = usage.get("input").and_then(|x| x.as_i64()).unwrap_or(0);
        let output = usage.get("output").and_then(|x| x.as_i64()).unwrap_or(0);
        let cache_read = usage
            .get("cacheRead")
            .or_else(|| usage.get("cache_read"))
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let cache_write = usage
            .get("cacheWrite")
            .or_else(|| usage.get("cache_write"))
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        if input == 0 && output == 0 && cache_read == 0 && cache_write == 0 {
            continue;
        }
        let session_id = v
            .pointer("/message/sessionId")
            .or_else(|| v.get("sessionId"))
            .and_then(|x| x.as_str())
            .map(String::from)
            .unwrap_or_else(|| file_session.to_string());
        let call_id = v
            .get("id")
            .and_then(|x| x.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("{file_session}:{i}"));
        let ts = v
            .get("timestamp")
            .and_then(parse_ts)
            .unwrap_or(ingested_at);
        let model = msg
            .get("model")
            .or_else(|| msg.get("modelId"))
            .and_then(|x| x.as_str())
            .map(String::from);
        let provider = msg.get("provider").and_then(|x| x.as_str()).map(String::from);
        let mut ev = UsageEvent::new_base("pi", PlatformKind::Cli, &session_id, &source, ingested_at);
        ev.id = make_event_id("pi", &format!("{source}:{call_id}"));
        ev.call_id = Some(call_id);
        ev.surface = Some("cli".into());
        ev.ts = ts;
        ev.model = model;
        ev.provider = provider;
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

fn parse_ts(v: &Value) -> Option<i64> {
    if let Some(n) = v.as_i64() {
        return Some(if n < 10_000_000_000 { n * 1000 } else { n });
    }
    v.as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parses_pi_message_usage() {
        let line = r#"{"type":"message","id":"abc","timestamp":"2026-05-19T10:06:03.191Z","message":{"role":"assistant","model":"qwen3","provider":"aliyun","usage":{"input":10,"output":5,"cacheRead":100,"cacheWrite":0}}}"#;
        let events = parse_pi_jsonl(line, Path::new("/tmp/s.jsonl"), "sess", 0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].input_tokens, 10);
        assert_eq!(events[0].output_tokens, 5);
        assert_eq!(events[0].cache_read_tokens, 100);
    }
}
