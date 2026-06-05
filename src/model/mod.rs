use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlatformKind {
    Cli,
    Ide,
    Hybrid,
    Server,
}

impl PlatformKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Ide => "ide",
            Self::Hybrid => "hybrid",
            Self::Server => "server",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cli" => Some(Self::Cli),
            "ide" => Some(Self::Ide),
            "hybrid" => Some(Self::Hybrid),
            "server" => Some(Self::Server),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UsageQuality {
    Exact,
    Estimated,
    Credits,
}

impl UsageQuality {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Estimated => "estimated",
            Self::Credits => "credits",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    pub id: String,
    pub platform: String,
    pub platform_kind: PlatformKind,
    pub surface: Option<String>,
    pub session_id: String,
    pub call_id: Option<String>,
    pub ts: i64,
    pub project_path: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
    pub total_tokens: i64,
    pub cost_usd: Option<f64>,
    pub usage_unit: String,
    pub quality: UsageQuality,
    pub source_path: String,
    pub ingested_at: i64,
}

impl UsageEvent {
    pub fn compute_total(&mut self) {
        if self.total_tokens == 0 {
            self.total_tokens = self.input_tokens
                + self.output_tokens
                + self.cache_read_tokens
                + self.cache_write_tokens
                + self.reasoning_tokens;
        }
    }

    pub fn new_base(
        platform: &str,
        platform_kind: PlatformKind,
        session_id: &str,
        source_path: &str,
        ingested_at: i64,
    ) -> Self {
        Self {
            id: String::new(),
            platform: platform.to_string(),
            platform_kind,
            surface: None,
            session_id: session_id.to_string(),
            call_id: None,
            ts: ingested_at,
            project_path: None,
            model: None,
            provider: None,
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            total_tokens: 0,
            cost_usd: None,
            usage_unit: "tokens".to_string(),
            quality: UsageQuality::Exact,
            source_path: source_path.to_string(),
            ingested_at,
        }
    }
}

pub fn make_event_id(platform: &str, dedupe_key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(platform.as_bytes());
    hasher.update(b":");
    hasher.update(dedupe_key.as_bytes());
    format!("{platform}:{}", hex::encode(&hasher.finalize()[..16]))
}
