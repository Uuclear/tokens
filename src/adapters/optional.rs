use crate::db::TokenStore;
use crate::adapters::optional_api;
use crate::model::UsageEvent;
use anyhow::Result;

pub fn scan_optional(store: &TokenStore) -> Result<Vec<UsageEvent>> {
    let ingested_at = chrono::Utc::now().timestamp_millis();
    let mut events = Vec::new();

    events.extend(optional_api::scan_postman(store, ingested_at)?);
    events.extend(optional_api::scan_dify(store, ingested_at)?);
    events.extend(optional_api::scan_cursor_api(store, ingested_at)?);

    Ok(events)
}
