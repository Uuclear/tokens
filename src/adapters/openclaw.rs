use super::{Adapter, ProbeHit};
use crate::scan_filter::ScanFilter;
use crate::model::{make_event_id, PlatformKind, UsageEvent, UsageQuality};
use crate::paths::openclaw_state_dir;
use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub struct OpenClawAdapter;

impl Adapter for OpenClawAdapter {
    fn id(&self) -> &'static str {
        "openclaw"
    }

    fn probe(&self) -> Result<Vec<ProbeHit>> {
        let agents = openclaw_state_dir().join("agents");
        Ok(vec![ProbeHit {
            path: agents.display().to_string(),
            exists: agents.exists(),
            size_bytes: None,
            note: Some("agents/*/sessions/*.jsonl".into()),
        }])
    }

    fn scan(&self, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>> {
        let mut events = Vec::new();
        let agents_root = openclaw_state_dir().join("agents");
        if !agents_root.exists() {
            return Ok(events);
        }
        for entry in WalkDir::new(&agents_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x == "jsonl")
                    && e.path().parent().is_some_and(|p| {
                        p.file_name().is_some_and(|n| n == "sessions")
                    })
            })
        {
            let path = entry.path();
            if !filter.should_parse(path)? {
                continue;
            }
            let agent_id = path
                .ancestors()
                .find(|p| p.parent().is_some_and(|pp| pp.ends_with("agents")))
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("default");
            let session_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let content = fs::read_to_string(path)?;
            events.extend(parse_openclaw_jsonl(
                &content,
                path,
                agent_id,
                &session_id,
                ingested_at,
            ));
        }
        Ok(events)
    }
}

fn parse_openclaw_jsonl(
    content: &str,
    path: &Path,
    agent_id: &str,
    session_id: &str,
    ingested_at: i64,
) -> Vec<UsageEvent> {
    let mut events = Vec::new();
    let source = path.display().to_string();
    for (i, line) in content.lines().enumerate() {
        let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("message") {
            continue;
        }
        let role = v.pointer("/message/role").and_then(|r| r.as_str());
        if role != Some("assistant") {
            continue;
        }
        let usage = v.pointer("/message/usage");
        let Some(usage) = usage else { continue };
        let input = usage.get("input").and_then(|x| x.as_i64()).unwrap_or(0)
            + usage
                .get("inputTokens")
                .and_then(|x| x.as_i64())
                .unwrap_or(0)
            + usage
                .get("input_tokens")
                .and_then(|x| x.as_i64())
                .unwrap_or(0);
        let output = usage.get("output").and_then(|x| x.as_i64()).unwrap_or(0)
            + usage
                .get("outputTokens")
                .and_then(|x| x.as_i64())
                .unwrap_or(0)
            + usage
                .get("output_tokens")
                .and_then(|x| x.as_i64())
                .unwrap_or(0);
        if input == 0 && output == 0 {
            continue;
        }
        let cost = usage
            .pointer("/cost/total")
            .and_then(|x| x.as_f64());
        let id = v
            .pointer("/message/id")
            .and_then(|x| x.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("{i}"));
        let mut ev =
            UsageEvent::new_base("openclaw", PlatformKind::Cli, session_id, &source, ingested_at);
        ev.id = make_event_id("openclaw", &format!("{source}:{id}"));
        ev.call_id = Some(id);
        ev.surface = Some(format!("agent:{agent_id}"));
        ev.ts = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.timestamp_millis())
            .unwrap_or(ingested_at);
        ev.input_tokens = input;
        ev.output_tokens = output;
        ev.cost_usd = cost;
        ev.quality = UsageQuality::Exact;
        ev.compute_total();
        events.push(ev);
    }
    events
}
