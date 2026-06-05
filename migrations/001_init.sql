-- Multi-platform token usage schema (common denominator)

CREATE TABLE IF NOT EXISTS platforms (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('cli', 'ide', 'hybrid', 'server')),
    default_paths_windows TEXT,
    doc_url TEXT,
    adapter_version TEXT NOT NULL DEFAULT '0',
    status TEXT NOT NULL CHECK (status IN ('implemented', 'research', 'optional_api', 'stub'))
);

CREATE TABLE IF NOT EXISTS usage_events (
    id TEXT PRIMARY KEY,
    platform TEXT NOT NULL,
    platform_kind TEXT NOT NULL,
    surface TEXT,
    session_id TEXT NOT NULL,
    call_id TEXT,
    ts INTEGER NOT NULL,
    project_path TEXT,
    model TEXT,
    provider TEXT,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens INTEGER NOT NULL DEFAULT 0,
    cache_write_tokens INTEGER NOT NULL DEFAULT 0,
    reasoning_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    cost_usd REAL,
    usage_unit TEXT NOT NULL DEFAULT 'tokens' CHECK (usage_unit IN ('tokens', 'credits')),
    quality TEXT NOT NULL DEFAULT 'exact' CHECK (quality IN ('exact', 'estimated', 'credits')),
    source_path TEXT NOT NULL,
    ingested_at INTEGER NOT NULL,
    FOREIGN KEY (platform) REFERENCES platforms(id)
);

CREATE INDEX IF NOT EXISTS idx_usage_events_platform ON usage_events(platform);
CREATE INDEX IF NOT EXISTS idx_usage_events_ts ON usage_events(ts);
CREATE INDEX IF NOT EXISTS idx_usage_events_session ON usage_events(session_id);
CREATE INDEX IF NOT EXISTS idx_usage_events_project ON usage_events(project_path);

CREATE TABLE IF NOT EXISTS ingest_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at INTEGER NOT NULL,
    finished_at INTEGER,
    platform TEXT,
    files_scanned INTEGER NOT NULL DEFAULT 0,
    events_inserted INTEGER NOT NULL DEFAULT 0,
    events_skipped INTEGER NOT NULL DEFAULT 0,
    error_message TEXT
);

CREATE TABLE IF NOT EXISTS source_fingerprints (
    source_path TEXT NOT NULL,
    platform TEXT NOT NULL,
    mtime_secs INTEGER NOT NULL,
    size_bytes INTEGER NOT NULL,
    last_ingested_at INTEGER NOT NULL,
    PRIMARY KEY (source_path, platform)
);

CREATE TABLE IF NOT EXISTS app_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE VIEW IF NOT EXISTS v_daily_summary AS
SELECT
    date(ts / 1000, 'unixepoch', 'localtime') AS day,
    platform,
    platform_kind,
    COUNT(*) AS call_count,
    SUM(input_tokens) AS input_tokens,
    SUM(output_tokens) AS output_tokens,
    SUM(cache_read_tokens) AS cache_read_tokens,
    SUM(cache_write_tokens) AS cache_write_tokens,
    SUM(reasoning_tokens) AS reasoning_tokens,
    SUM(total_tokens) AS total_tokens,
    SUM(cost_usd) AS cost_usd
FROM usage_events
GROUP BY day, platform, platform_kind;

CREATE VIEW IF NOT EXISTS v_by_platform AS
SELECT
    platform,
    platform_kind,
    COUNT(DISTINCT session_id) AS session_count,
    COUNT(*) AS call_count,
    SUM(input_tokens) AS input_tokens,
    SUM(output_tokens) AS output_tokens,
    SUM(total_tokens) AS total_tokens,
    SUM(cost_usd) AS cost_usd
FROM usage_events
GROUP BY platform, platform_kind;

CREATE VIEW IF NOT EXISTS v_by_project AS
SELECT
    COALESCE(project_path, '(unknown)') AS project_path,
    platform,
    COUNT(*) AS call_count,
    SUM(total_tokens) AS total_tokens,
    SUM(cost_usd) AS cost_usd
FROM usage_events
GROUP BY project_path, platform;
