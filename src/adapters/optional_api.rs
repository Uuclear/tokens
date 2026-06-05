//! Optional cloud/API adapters (feature-gated).

use crate::db::TokenStore;
use crate::model::UsageEvent;
use anyhow::Result;

#[cfg(feature = "cursor_api")]
use crate::model::{make_event_id, PlatformKind, UsageQuality};
#[cfg(feature = "cursor_api")]
use anyhow::Context;
#[cfg(feature = "cursor_api")]
use serde_json::Value;
#[cfg(feature = "cursor_api")]
use std::time::Duration;

pub fn scan_postman(store: &TokenStore, _ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let api_key = store.get_config("postman_api_key")?;
    let Some(api_key) = api_key else {
        return Ok(Vec::new());
    };

    #[cfg(feature = "postman")]
    {
        return fetch_postman_credits(&api_key, _ingested_at);
    }

    #[cfg(not(feature = "postman"))]
    {
        let _ = api_key;
        return Ok(Vec::new());
    }
}

pub fn scan_dify(store: &TokenStore, _ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let api_url = store.get_config("dify_api_url")?;
    let api_key = store.get_config("dify_api_key")?;
    let (Some(url), Some(key)) = (api_url, api_key) else {
        return Ok(Vec::new());
    };

    #[cfg(feature = "dify")]
    {
        return fetch_dify_logs(&url, &key, _ingested_at);
    }

    #[cfg(not(feature = "dify"))]
    {
        let _ = (url, key);
        return Ok(Vec::new());
    }
}

pub fn scan_cursor_api(store: &TokenStore, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let token = resolve_cursor_session_token(store)?;
    let Some(token) = token else {
        return Ok(Vec::new());
    };

    #[cfg(feature = "cursor_api")]
    {
        return fetch_cursor_usage_events(&token, ingested_at);
    }

    #[cfg(not(feature = "cursor_api"))]
    {
        let _ = (token, ingested_at);
        return Ok(Vec::new());
    }
}

/// `cursor_session_token` config, or `CURSOR_SESSION_TOKEN` env (WorkosCursorSessionToken value).
pub fn resolve_cursor_session_token(store: &TokenStore) -> Result<Option<String>> {
    if let Some(t) = store.get_config("cursor_session_token")? {
        if !t.trim().is_empty() {
            return Ok(Some(t));
        }
    }
    if let Ok(t) = std::env::var("CURSOR_SESSION_TOKEN") {
        if !t.trim().is_empty() {
            return Ok(Some(t));
        }
    }
    Ok(None)
}

#[cfg(feature = "cursor_api")]
fn fetch_cursor_usage_events(session_token: &str, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let rt = tokio::runtime::Runtime::new().context("tokio runtime for cursor API")?;
    rt.block_on(fetch_cursor_usage_events_async(session_token, ingested_at))
}

#[cfg(feature = "cursor_api")]
async fn fetch_cursor_usage_events_async(
    session_token: &str,
    ingested_at: i64,
) -> Result<Vec<UsageEvent>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;
    let cookie = format!("WorkosCursorSessionToken={session_token}");
    let url = "https://cursor.com/api/dashboard/get-filtered-usage-events";

    let mut events = Vec::new();
    let page_size = 100usize;
    let mut page = 1usize;
    let mut total_count: Option<usize> = None;

    loop {
        let body = serde_json::json!({
            "page": page,
            "pageSize": page_size,
        });
        let resp = client
            .post(url)
            .header("Cookie", &cookie)
            .header("Origin", "https://cursor.com")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .with_context(|| format!("cursor API page {page}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("cursor API HTTP {status}: {text}");
        }

        let v: Value = resp.json().await?;
        if total_count.is_none() {
            total_count = v
                .get("totalUsageEventsCount")
                .and_then(|x| x.as_u64())
                .map(|n| n as usize);
        }
        let batch = v
            .get("usageEventsDisplay")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();

        if batch.is_empty() {
            break;
        }

        for (idx, item) in batch.iter().enumerate() {
            if let Some(ev) = parse_cursor_api_event(item, page, idx, ingested_at) {
                events.push(ev);
            }
        }

        let fetched = page * page_size;
        if let Some(total) = total_count {
            if fetched >= total {
                break;
            }
        }
        if batch.len() < page_size {
            break;
        }
        page += 1;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    Ok(events)
}

#[cfg(feature = "cursor_api")]
fn parse_cursor_api_event(
    item: &Value,
    page: usize,
    idx: usize,
    ingested_at: i64,
) -> Option<UsageEvent> {
    let usage = item.get("tokenUsage")?;
    let input = usage.get("inputTokens").and_then(|x| x.as_i64()).unwrap_or(0);
    let output = usage.get("outputTokens").and_then(|x| x.as_i64()).unwrap_or(0);
    let cache_write = usage
        .get("cacheWriteTokens")
        .or_else(|| usage.get("cache_write_tokens"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let cache_read = usage
        .get("cacheReadTokens")
        .or_else(|| usage.get("cache_read_tokens"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    if input == 0 && output == 0 && cache_read == 0 && cache_write == 0 {
        return None;
    }
    let ts = item
        .get("timestamp")
        .and_then(|x| x.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(ingested_at);
    let model = item.get("model").and_then(|x| x.as_str()).map(String::from);
    let event_id = item
        .get("requestId")
        .or_else(|| item.get("id"))
        .and_then(|x| x.as_str())
        .map(String::from)
        .unwrap_or_else(|| format!("api:{page}:{idx}:{ts}"));
    let source = "cursor.com/api/dashboard/get-filtered-usage-events".to_string();
    let mut ev = UsageEvent::new_base("cursor", PlatformKind::Cli, "cursor-api", &source, ingested_at);
    ev.id = make_event_id("cursor", &format!("api:{event_id}"));
    ev.call_id = Some(event_id);
    ev.surface = Some("api".into());
    ev.ts = ts;
    ev.model = model;
    ev.input_tokens = input;
    ev.output_tokens = output;
    ev.cache_read_tokens = cache_read;
    ev.cache_write_tokens = cache_write;
    if let Some(cents) = item.get("chargedCents").and_then(|x| x.as_f64()) {
        ev.cost_usd = Some(cents / 100.0);
    } else if let Some(cents) = usage.get("totalCents").and_then(|x| x.as_f64()) {
        ev.cost_usd = Some(cents / 100.0);
    }
    ev.quality = UsageQuality::Exact;
    ev.compute_total();
    Some(ev)
}

#[cfg(feature = "postman")]
fn fetch_postman_credits(api_key: &str, _ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let _ = api_key;
    Ok(Vec::new())
}

#[cfg(feature = "dify")]
fn fetch_dify_logs(api_url: &str, api_key: &str, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let _ = (api_url, api_key, ingested_at);
    Ok(Vec::new())
}
