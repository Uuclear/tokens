use super::{Adapter, ProbeHit};
use crate::scan_filter::ScanFilter;
use crate::model::{make_event_id, PlatformKind, UsageEvent, UsageQuality};
use crate::paths::claude_config_dirs;
use crate::paths::discovery::{claude_all_roots, ClaudeRoot};
use anyhow::Result;
use chrono::{NaiveDate, TimeZone, Utc};
use serde_json::Value;
use std::collections::HashMap;
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
        for dir in claude_config_dirs() {
            let cache = dir.join("stats-cache.json");
            hits.push(ProbeHit {
                path: cache.display().to_string(),
                exists: cache.exists(),
                size_bytes: fs::metadata(&cache).ok().map(|m| m.len()),
                note: Some("native /usage and /stats totals".into()),
            });
        }
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

    fn scan(&self, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>> {
        let mut events = Vec::new();
        for dir in claude_config_dirs() {
            let cache_path = dir.join("stats-cache.json");
            if cache_path.exists() && filter.should_parse(&cache_path)? {
                events.extend(scan_stats_cache(&cache_path, ingested_at)?);
            }
        }
        if !events.is_empty() {
            return Ok(events);
        }

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
                if !filter.should_parse(path)? {
                    continue;
                }
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

/// Claude Code `/usage` and `/stats` read pre-aggregated totals from stats-cache.json.
fn scan_stats_cache(path: &Path, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let content = fs::read_to_string(path)?;
    let v: Value = serde_json::from_str(&content)?;
    let daily = v.get("dailyModelTokens").and_then(|x| x.as_array());
    let model_usage = v.get("modelUsage").and_then(|x| x.as_object());
    let (Some(daily), Some(model_usage)) = (daily, model_usage) else {
        return Ok(Vec::new());
    };

    let ratios = model_input_ratio(model_usage);
    let source = path.display().to_string();
    let mut events = Vec::new();

    for day in daily {
        let date = day.get("date").and_then(|x| x.as_str()).unwrap_or_default();
        let Some(tokens_by_model) = day.get("tokensByModel").and_then(|x| x.as_object()) else {
            continue;
        };
        let ts = date_to_millis(date).unwrap_or(ingested_at);
        for (model, total_val) in tokens_by_model {
            let total = total_val.as_i64().unwrap_or(0);
            if total <= 0 {
                continue;
            }
            let ratio = ratios.get(model.as_str()).copied().unwrap_or(1.0);
            let input = (total as f64 * ratio).round() as i64;
            let output = total - input;
            let mut ev = UsageEvent::new_base(
                "claude_code",
                PlatformKind::Cli,
                &format!("stats-cache:{date}"),
                &source,
                ingested_at,
            );
            ev.id = make_event_id("claude_code", &format!("{source}:{date}:{model}"));
            ev.call_id = Some(format!("{date}:{model}"));
            ev.surface = Some("cli".into());
            ev.ts = ts;
            ev.model = Some(model.clone());
            ev.input_tokens = input;
            ev.output_tokens = output;
            ev.quality = UsageQuality::Exact;
            ev.compute_total();
            events.push(ev);
        }
    }
    Ok(events)
}

fn model_input_ratio(model_usage: &serde_json::Map<String, Value>) -> HashMap<String, f64> {
    let mut ratios = HashMap::new();
    for (model, usage) in model_usage {
        let input = usage
            .get("inputTokens")
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let output = usage
            .get("outputTokens")
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let total = input + output;
        let ratio = if total > 0 {
            input as f64 / total as f64
        } else {
            1.0
        };
        ratios.insert(model.clone(), ratio);
    }
    ratios
}

fn date_to_millis(date: &str) -> Option<i64> {
    let naive = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let dt = naive.and_hms_opt(12, 0, 0)?;
    Some(Utc.from_utc_datetime(&dt).timestamp_millis())
}

fn parse_jsonl(
    content: &str,
    path: &Path,
    file_session: &str,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn stats_cache_emits_daily_model_events() {
        let dir = tempdir().unwrap();
        let cache = dir.path().join("stats-cache.json");
        let body = r#"{
            "dailyModelTokens": [
                {"date": "2026-05-06", "tokensByModel": {"qwen3.6-plus": 1000}}
            ],
            "modelUsage": {
                "qwen3.6-plus": {"inputTokens": 900, "outputTokens": 100}
            }
        }"#;
        let mut f = fs::File::create(&cache).unwrap();
        f.write_all(body.as_bytes()).unwrap();

        let events = scan_stats_cache(&cache, 0).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].input_tokens, 900);
        assert_eq!(events[0].output_tokens, 100);
        assert_eq!(events[0].model.as_deref(), Some("qwen3.6-plus"));
    }
}
