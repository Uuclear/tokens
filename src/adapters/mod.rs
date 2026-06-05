mod chatbox;
mod cherry_studio;
mod claude_code;
mod cline;
mod codex;
mod cursor;
mod hermes;
mod kilo;
mod openclaw;
mod opencode;
mod pi;
pub mod optional;
mod optional_api;
mod qoder;
mod qwen_code;

use crate::scan_filter::ScanFilter;
use crate::model::UsageEvent;
use anyhow::Result;

pub trait Adapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn scan(&self, ingested_at: i64, filter: &ScanFilter) -> Result<Vec<UsageEvent>>;
    fn probe(&self) -> Result<Vec<ProbeHit>>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProbeHit {
    pub path: String,
    pub exists: bool,
    pub size_bytes: Option<u64>,
    pub note: Option<String>,
}

pub fn all_adapters() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(claude_code::ClaudeCodeAdapter),
        Box::new(codex::CodexAdapter),
        Box::new(opencode::OpenCodeAdapter),
        Box::new(openclaw::OpenClawAdapter),
        Box::new(hermes::HermesAdapter),
        Box::new(pi::PiAdapter),
        Box::new(cline::ClineAdapter),
        Box::new(kilo::KiloCliAdapter),
        Box::new(kilo::KiloIdeAdapter),
        Box::new(cursor::CursorAdapter),
        Box::new(qwen_code::QwenCodeAdapter),
        Box::new(cherry_studio::CherryStudioAdapter),
        Box::new(chatbox::ChatboxAdapter),
        Box::new(qoder::QoderAdapter::new("qoder")),
        Box::new(qoder::QoderAdapter::new("qoder_cn")),
    ]
}

pub fn adapter_by_id(id: &str) -> Option<Box<dyn Adapter>> {
    all_adapters().into_iter().find(|a| a.id() == id)
}

pub fn optional_adapters(store: &crate::db::TokenStore) -> Result<Vec<UsageEvent>> {
    optional::scan_optional(store)
}
