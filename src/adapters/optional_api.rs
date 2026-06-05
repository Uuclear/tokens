//! Optional cloud/API adapters (feature-gated).

use crate::db::TokenStore;
use crate::model::UsageEvent;
use anyhow::Result;

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
        Ok(Vec::new())
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
        Ok(Vec::new())
    }
}

pub fn scan_cursor_api(store: &TokenStore, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let token = store.get_config("cursor_session_token")?;
    if token.is_none() {
        return Ok(Vec::new());
    }

    #[cfg(feature = "cursor_api")]
    {
        // Cursor usage API sync placeholder — store local cache path in config when implemented
        let _ = ingested_at;
        Ok(Vec::new())
    }

    #[cfg(not(feature = "cursor_api"))]
    {
        let _ = ingested_at;
        Ok(Vec::new())
    }
}

#[cfg(feature = "postman")]
fn fetch_postman_credits(api_key: &str, _ingested_at: i64) -> Result<Vec<UsageEvent>> {
    // Postman Agent Mode reports use AI credits; extend with workspace-scoped API when configured.
    let _ = api_key;
    Ok(Vec::new())
}

#[cfg(feature = "dify")]
fn fetch_dify_logs(api_url: &str, api_key: &str, ingested_at: i64) -> Result<Vec<UsageEvent>> {
    let _ = (api_url, api_key, ingested_at);
    Ok(Vec::new())
}
