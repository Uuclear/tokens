use super::{Adapter, ProbeHit};
use crate::scan_filter::ScanFilter;
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
            note: Some("~/.qwen/projects — chats/*.jsonl with usageMetadata".into()),
        }])
    }

    fn scan(&self, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>> {
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
            if !filter.should_parse(path)? {
                continue;
            }
            let content = fs::read_to_string(path)?;
            if path.extension().is_some_and(|x| x == "jsonl") {
                for (i, line) in content.lines().enumerate() {
                    if let Ok(v) = serde_json::from_str::<Value>(line) {
                        let fallback_id = format!("line-{i}");
                        let session_id = v
                            .get("sessionId")
                            .and_then(|s| s.as_str())
                            .unwrap_or(fallback_id.as_str());
                        if let Some(ev) = extract_usage(&v, path, session_id, ingested_at, i) {
                            events.push(ev);
                        }
                    }
                }
            } else if let Ok(v) = serde_json::from_str::<Value>(&content) {
                if let Some(ev) = extract_usage(&v, path, "file", ingested_at, 0) {
                    events.push(ev);
                }
                if let Some(usage) = v.pointer("/usage") {
                    if let Some(ev) = extract_usage_from_usage_obj(
                        usage,
                        path,
                        v.get("sessionId").and_then(|s| s.as_str()).unwrap_or("file"),
                        ingested_at,
                        None,
                        0,
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
    session_id: &str,
    ingested_at: i64,
    line_idx: usize,
) -> Option<UsageEvent> {
    if let Some(usage) = v
        .get("usage")
        .or_else(|| v.pointer("/stats/usage"))
        .or_else(|| v.get("tokenUsage"))
    {
        return extract_usage_from_usage_obj(
            usage,
            path,
            session_id,
            ingested_at,
            v.get("model").and_then(|m| m.as_str()),
            line_idx,
        );
    }
    if let Some(meta) = v.get("usageMetadata") {
        return extract_usage_from_metadata(
            meta,
            path,
            session_id,
            ingested_at,
            v.get("model").and_then(|m| m.as_str()),
            line_idx,
        );
    }
    None
}

fn extract_usage_from_metadata(
    meta: &Value,
    path: &std::path::Path,
    session_id: &str,
    ingested_at: i64,
    model: Option<&str>,
    line_idx: usize,
) -> Option<UsageEvent> {
    let input = meta
        .get("promptTokenCount")
        .or_else(|| meta.get("inputTokens"))
        .or_else(|| meta.get("input_tokens"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let output = meta
        .get("candidatesTokenCount")
        .or_else(|| meta.get("responseTokenCount"))
        .or_else(|| meta.get("outputTokens"))
        .or_else(|| meta.get("output_tokens"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let reasoning = meta
        .get("thoughtsTokenCount")
        .or_else(|| meta.get("reasoning_tokens"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let cache_read = meta
        .get("cachedContentTokenCount")
        .or_else(|| meta.get("cache_read_tokens"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    if input == 0 && output == 0 && reasoning == 0 && cache_read == 0 {
        return None;
    }
    let source = path.display().to_string();
    let mut ev =
        UsageEvent::new_base("qwen_code", PlatformKind::Cli, session_id, &source, ingested_at);
    ev.id = make_event_id(
        "qwen_code",
        &format!("{source}:{session_id}:{line_idx}:{input}:{output}"),
    );
    ev.call_id = Some(format!("{session_id}:{line_idx}"));
    ev.input_tokens = input;
    ev.output_tokens = output;
    ev.reasoning_tokens = reasoning;
    ev.cache_read_tokens = cache_read;
    ev.model = model.map(String::from);
    ev.quality = UsageQuality::Exact;
    ev.compute_total();
    Some(ev)
}

fn extract_usage_from_usage_obj(
    usage: &Value,
    path: &std::path::Path,
    session_id: &str,
    ingested_at: i64,
    model: Option<&str>,
    line_idx: usize,
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
    ev.id = make_event_id(
        "qwen_code",
        &format!("{source}:{session_id}:{line_idx}:{input}:{output}"),
    );
    ev.call_id = Some(format!("{session_id}:{line_idx}"));
    ev.input_tokens = input;
    ev.output_tokens = output;
    ev.model = model
        .map(String::from)
        .or_else(|| usage.get("model").and_then(|m| m.as_str()).map(String::from));
    ev.quality = UsageQuality::Exact;
    ev.compute_total();
    Some(ev)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parses_usage_metadata() {
        let v: Value = serde_json::from_str(
            r#"{"sessionId":"s1","model":"qwen3","usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":20,"thoughtsTokenCount":5,"cachedContentTokenCount":10}}"#,
        )
        .unwrap();
        let ev = extract_usage(&v, Path::new("/tmp/chat.jsonl"), "s1", 0, 0).unwrap();
        assert_eq!(ev.input_tokens, 100);
        assert_eq!(ev.output_tokens, 20);
        assert_eq!(ev.reasoning_tokens, 5);
        assert_eq!(ev.cache_read_tokens, 10);
        assert_eq!(ev.total_tokens, 135);
    }
}
