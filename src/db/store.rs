use crate::model::UsageEvent;
use crate::registry::PlatformRegistry;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

const MIGRATION_SQL: &str = include_str!("../../migrations/001_init.sql");

pub struct TokenStore {
    conn: Connection,
}

impl TokenStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("open database {}", path.display()))?;
        conn.execute_batch(MIGRATION_SQL)?;
        Ok(Self { conn })
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn seed_platforms(&self, registry: &PlatformRegistry) -> Result<()> {
        for p in &registry.platforms {
            let paths = p
                .paths
                .as_ref()
                .map(|pp| {
                    pp.templates_for_host()
                        .join(";")
                })
                .unwrap_or_default();
            self.conn.execute(
                "INSERT OR REPLACE INTO platforms (id, display_name, kind, default_paths_windows, doc_url, adapter_version, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    p.id,
                    p.display_name,
                    p.kind,
                    paths,
                    p.doc_url,
                    p.adapter_version,
                    p.status,
                ],
            )?;
        }
        Ok(())
    }

    pub fn should_skip_source(
        &self,
        platform: &str,
        source_path: &str,
        mtime_secs: i64,
        size_bytes: i64,
        full_rescan: bool,
    ) -> Result<bool> {
        if full_rescan {
            return Ok(false);
        }
        let existing: Option<(i64, i64)> = self.conn.query_row(
            "SELECT mtime_secs, size_bytes FROM source_fingerprints WHERE source_path = ?1 AND platform = ?2",
            params![source_path, platform],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).ok();
        Ok(matches!(existing, Some((m, s)) if m == mtime_secs && s == size_bytes))
    }

    pub fn upsert_fingerprint(
        &self,
        platform: &str,
        source_path: &str,
        mtime_secs: i64,
        size_bytes: i64,
        ingested_at: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO source_fingerprints (source_path, platform, mtime_secs, size_bytes, last_ingested_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![source_path, platform, mtime_secs, size_bytes, ingested_at],
        )?;
        Ok(())
    }

    pub fn insert_event(&self, event: &UsageEvent) -> Result<bool> {
        let rows = self.conn.execute(
            "INSERT OR IGNORE INTO usage_events (
                id, platform, platform_kind, surface, session_id, call_id, ts,
                project_path, model, provider,
                input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, reasoning_tokens, total_tokens,
                cost_usd, usage_unit, quality, source_path, ingested_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21
            )",
            params![
                event.id,
                event.platform,
                event.platform_kind.as_str(),
                event.surface,
                event.session_id,
                event.call_id,
                event.ts,
                event.project_path,
                event.model,
                event.provider,
                event.input_tokens,
                event.output_tokens,
                event.cache_read_tokens,
                event.cache_write_tokens,
                event.reasoning_tokens,
                event.total_tokens,
                event.cost_usd,
                event.usage_unit,
                event.quality.as_str(),
                event.source_path,
                event.ingested_at,
            ],
        )?;
        Ok(rows > 0)
    }

    pub fn start_ingest_run(&self, platform: Option<&str>) -> Result<i64> {
        let now = chrono::Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT INTO ingest_runs (started_at, platform) VALUES (?1, ?2)",
            params![now, platform],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn finish_ingest_run(
        &self,
        run_id: i64,
        files: i64,
        inserted: i64,
        skipped: i64,
        error: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        self.conn.execute(
            "UPDATE ingest_runs SET finished_at = ?1, files_scanned = ?2, events_inserted = ?3, events_skipped = ?4, error_message = ?5 WHERE id = ?6",
            params![now, files, inserted, skipped, error, run_id],
        )?;
        Ok(())
    }

    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT value FROM app_config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO app_config (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn delete_config(&self, key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM app_config WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn list_config_keys(&self, prefix: Option<&str>) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        match prefix {
            Some(p) => {
                let pattern = format!("{p}%");
                let mut stmt = self
                    .conn
                    .prepare("SELECT key FROM app_config WHERE key LIKE ?1 ORDER BY key")?;
                let rows = stmt.query_map(params![pattern], |row| row.get::<_, String>(0))?;
                for row in rows {
                    keys.push(row?);
                }
            }
            None => {
                let mut stmt = self.conn.prepare("SELECT key FROM app_config ORDER BY key")?;
                let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
                for row in rows {
                    keys.push(row?);
                }
            }
        }
        Ok(keys)
    }

    pub fn report_by_platform(&self, since_ms: Option<i64>) -> Result<Vec<ReportRow>> {
        self.report_grouped(
            since_ms,
            "platform, platform_kind",
            "platform, platform_kind",
        )
    }

    pub fn report_by_platform_surface(&self, since_ms: Option<i64>) -> Result<Vec<SurfaceReportRow>> {
        let sql = if since_ms.is_some() {
            "SELECT platform, platform_kind, COALESCE(surface, '(default)'),
                    COUNT(DISTINCT session_id), COUNT(*),
                    SUM(input_tokens), SUM(output_tokens),
                    SUM(cache_read_tokens), SUM(cache_write_tokens), SUM(reasoning_tokens),
                    SUM(total_tokens)
             FROM usage_events WHERE ts >= ?1
             GROUP BY platform, platform_kind, COALESCE(surface, '(default)')
             ORDER BY SUM(total_tokens) DESC"
        } else {
            "SELECT platform, platform_kind, COALESCE(surface, '(default)'),
                    COUNT(DISTINCT session_id), COUNT(*),
                    SUM(input_tokens), SUM(output_tokens),
                    SUM(cache_read_tokens), SUM(cache_write_tokens), SUM(reasoning_tokens),
                    SUM(total_tokens)
             FROM usage_events
             GROUP BY platform, platform_kind, COALESCE(surface, '(default)')
             ORDER BY SUM(total_tokens) DESC"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = if let Some(since) = since_ms {
            stmt.query_map(params![since], map_surface_row)?
        } else {
            stmt.query_map([], map_surface_row)?
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn top_models(
        &self,
        since_ms: Option<i64>,
        platform: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ModelReportRow>> {
        let mut sql = String::from(
            "SELECT COALESCE(model, '(unknown)'), platform, COUNT(*),
                    SUM(input_tokens), SUM(output_tokens), SUM(total_tokens)
             FROM usage_events",
        );
        if since_ms.is_some() {
            sql.push_str(" WHERE ts >= ?1");
        }
        if platform.is_some() {
            sql.push_str(if since_ms.is_some() {
                " AND platform = ?2"
            } else {
                " WHERE platform = ?1"
            });
        }
        sql.push_str(" GROUP BY model, platform ORDER BY SUM(total_tokens) DESC LIMIT ");
        sql.push_str(&limit.to_string());
        let mut stmt = self.conn.prepare(&sql)?;
        match (since_ms, platform) {
            (Some(s), Some(p)) => {
                let rows = stmt.query_map(params![s, p], map_model_row)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
            }
            (Some(s), None) => {
                let rows = stmt.query_map(params![s], map_model_row)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
            }
            (None, Some(p)) => {
                let rows = stmt.query_map(params![p], map_model_row)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
            }
            (None, None) => {
                let rows = stmt.query_map([], map_model_row)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
            }
        }
    }

    pub fn session_stats(&self, since_ms: Option<i64>, platform: Option<&str>) -> Result<SessionStats> {
        let (where_clause, p1, p2): (String, Option<i64>, Option<String>) = match (since_ms, platform) {
            (Some(s), Some(p)) => (
                "WHERE ts >= ?1 AND platform = ?2".into(),
                Some(s),
                Some(p.to_string()),
            ),
            (Some(s), None) => ("WHERE ts >= ?1".into(), Some(s), None),
            (None, Some(p)) => ("WHERE platform = ?1".into(), None, Some(p.to_string())),
            (None, None) => (String::new(), None, None),
        };
        let sql = format!(
            "SELECT COUNT(DISTINCT session_id),
                    MIN(ts), MAX(ts)
             FROM usage_events {where_clause}"
        );
        let (sessions, min_ts, max_ts): (i64, Option<i64>, Option<i64>) = match (p1, p2) {
            (Some(s), Some(p)) => self.conn.query_row(
                &sql,
                params![s, p],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?,
            (Some(s), None) => self
                .conn
                .query_row(&sql, params![s], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?,
            (None, Some(p)) => self
                .conn
                .query_row(&sql, params![p], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?,
            (None, None) => self
                .conn
                .query_row(&sql, [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?,
        };
        let duration_secs = match (min_ts, max_ts) {
            (Some(a), Some(b)) if b > a => (b - a) / 1000,
            _ => 0,
        };
        let active_days = self.count_active_days(since_ms, platform)?;
        Ok(SessionStats {
            session_count: sessions,
            duration_secs,
            active_days,
        })
    }

    fn count_active_days(&self, since_ms: Option<i64>, platform: Option<&str>) -> Result<i64> {
        let (where_clause, p1, p2): (String, Option<i64>, Option<String>) =
            match (since_ms, platform) {
                (Some(s), Some(p)) => (
                    "WHERE ts >= ?1 AND platform = ?2".into(),
                    Some(s),
                    Some(p.to_string()),
                ),
                (Some(s), None) => ("WHERE ts >= ?1".into(), Some(s), None),
                (None, Some(p)) => ("WHERE platform = ?1".into(), None, Some(p.to_string())),
                (None, None) => (String::new(), None, None),
            };
        let sql = format!(
            "SELECT COUNT(DISTINCT strftime('%Y-%m-%d', ts / 1000, 'unixepoch'))
             FROM usage_events {where_clause}"
        );
        match (p1, p2) {
            (Some(s), Some(p)) => self
                .conn
                .query_row(&sql, params![s, p], |row| row.get(0))
                .map_err(Into::into),
            (Some(s), None) => self
                .conn
                .query_row(&sql, params![s], |row| row.get(0))
                .map_err(Into::into),
            (None, Some(p)) => self
                .conn
                .query_row(&sql, params![p], |row| row.get(0))
                .map_err(Into::into),
            (None, None) => self
                .conn
                .query_row(&sql, [], |row| row.get(0))
                .map_err(Into::into),
        }
    }

    fn report_grouped(
        &self,
        since_ms: Option<i64>,
        _group_cols: &str,
        order_group: &str,
    ) -> Result<Vec<ReportRow>> {
        let sql = if since_ms.is_some() {
            format!(
                "SELECT platform, platform_kind, COUNT(DISTINCT session_id), COUNT(*),
                    SUM(input_tokens), SUM(output_tokens),
                    SUM(cache_read_tokens), SUM(cache_write_tokens),
                    SUM(total_tokens), SUM(cost_usd)
             FROM usage_events WHERE ts >= ?1
             GROUP BY {order_group} ORDER BY SUM(total_tokens) DESC"
            )
        } else {
            format!(
                "SELECT platform, platform_kind, COUNT(DISTINCT session_id), COUNT(*),
                    SUM(input_tokens), SUM(output_tokens),
                    SUM(cache_read_tokens), SUM(cache_write_tokens),
                    SUM(total_tokens), SUM(cost_usd)
             FROM usage_events
             GROUP BY {order_group} ORDER BY SUM(total_tokens) DESC"
            )
        };
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = if let Some(since) = since_ms {
            stmt.query_map(params![since], map_report_row)?
        } else {
            stmt.query_map([], map_report_row)?
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn report_by_project(&self, since_ms: Option<i64>) -> Result<Vec<ProjectReportRow>> {
        let sql = if since_ms.is_some() {
            "SELECT COALESCE(project_path, '(unknown)'), platform, COUNT(*), SUM(total_tokens), SUM(cost_usd)
             FROM usage_events WHERE ts >= ?1
             GROUP BY project_path, platform ORDER BY SUM(total_tokens) DESC LIMIT 50"
        } else {
            "SELECT COALESCE(project_path, '(unknown)'), platform, COUNT(*), SUM(total_tokens), SUM(cost_usd)
             FROM usage_events
             GROUP BY project_path, platform ORDER BY SUM(total_tokens) DESC LIMIT 50"
        };
        let mut stmt = self.conn.prepare(sql)?;
        collect_project_rows(&mut stmt, since_ms)
    }

    pub fn report_by_model(&self, since_ms: Option<i64>) -> Result<Vec<ModelReportRow>> {
        let sql = if since_ms.is_some() {
            "SELECT COALESCE(model, '(unknown)'), platform, COUNT(*), SUM(input_tokens), SUM(output_tokens), SUM(total_tokens)
             FROM usage_events WHERE ts >= ?1
             GROUP BY model, platform ORDER BY SUM(total_tokens) DESC LIMIT 50"
        } else {
            "SELECT COALESCE(model, '(unknown)'), platform, COUNT(*), SUM(input_tokens), SUM(output_tokens), SUM(total_tokens)
             FROM usage_events
             GROUP BY model, platform ORDER BY SUM(total_tokens) DESC LIMIT 50"
        };
        let mut stmt = self.conn.prepare(sql)?;
        collect_model_rows(&mut stmt, since_ms)
    }

    pub fn event_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))?)
    }

    /// Daily token totals for charts (`day` = YYYY-MM-DD local).
    pub fn daily_totals(&self, since_ms: Option<i64>) -> Result<Vec<DailyTokenRow>> {
        let sql = if since_ms.is_some() {
            "SELECT date(ts / 1000, 'unixepoch', 'localtime') AS d, SUM(total_tokens)
             FROM usage_events WHERE ts >= ?1 GROUP BY d ORDER BY d"
        } else {
            "SELECT date(ts / 1000, 'unixepoch', 'localtime') AS d, SUM(total_tokens)
             FROM usage_events GROUP BY d ORDER BY d"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = match since_ms {
            Some(s) => stmt.query_map(params![s], map_daily_row)?,
            None => stmt.query_map([], map_daily_row)?,
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn daily_totals_by_platform(&self, since_ms: Option<i64>) -> Result<Vec<DailyPlatformRow>> {
        let sql = if since_ms.is_some() {
            "SELECT date(ts / 1000, 'unixepoch', 'localtime') AS d, platform, SUM(total_tokens)
             FROM usage_events WHERE ts >= ?1 GROUP BY d, platform ORDER BY d, platform"
        } else {
            "SELECT date(ts / 1000, 'unixepoch', 'localtime') AS d, platform, SUM(total_tokens)
             FROM usage_events GROUP BY d, platform ORDER BY d, platform"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = match since_ms {
            Some(s) => stmt.query_map(params![s], map_daily_platform_row)?,
            None => stmt.query_map([], map_daily_platform_row)?,
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Remove ingested events for one platform (used with `scan --full` to refresh parsing).
    pub fn delete_platform_events(&self, platform: &str) -> Result<i64> {
        let n = self.conn.execute(
            "DELETE FROM usage_events WHERE platform = ?1",
            params![platform],
        )?;
        self.conn.execute(
            "DELETE FROM source_fingerprints WHERE platform = ?1",
            params![platform],
        )?;
        Ok(n as i64)
    }
}

fn map_project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectReportRow> {
    Ok(ProjectReportRow {
        project_path: row.get(0)?,
        platform: row.get(1)?,
        call_count: row.get(2)?,
        total_tokens: row.get(3)?,
        cost_usd: row.get(4)?,
    })
}

fn collect_project_rows(
    stmt: &mut rusqlite::Statement<'_>,
    since_ms: Option<i64>,
) -> Result<Vec<ProjectReportRow>> {
    let rows = match since_ms {
        Some(since) => stmt.query_map(params![since], map_project_row)?,
        None => stmt.query_map([], map_project_row)?,
    };
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn map_model_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ModelReportRow> {
    Ok(ModelReportRow {
        model: row.get(0)?,
        platform: row.get(1)?,
        call_count: row.get(2)?,
        input_tokens: row.get(3)?,
        output_tokens: row.get(4)?,
        total_tokens: row.get(5)?,
    })
}

fn collect_model_rows(
    stmt: &mut rusqlite::Statement<'_>,
    since_ms: Option<i64>,
) -> Result<Vec<ModelReportRow>> {
    let rows = match since_ms {
        Some(since) => stmt.query_map(params![since], map_model_row)?,
        None => stmt.query_map([], map_model_row)?,
    };
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn map_report_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReportRow> {
    Ok(ReportRow {
        platform: row.get(0)?,
        platform_kind: row.get(1)?,
        session_count: row.get(2)?,
        call_count: row.get(3)?,
        input_tokens: row.get(4)?,
        output_tokens: row.get(5)?,
        cache_read_tokens: row.get(6)?,
        cache_write_tokens: row.get(7)?,
        total_tokens: row.get(8)?,
        cost_usd: row.get(9)?,
    })
}

fn map_surface_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SurfaceReportRow> {
    Ok(SurfaceReportRow {
        platform: row.get(0)?,
        platform_kind: row.get(1)?,
        surface: row.get(2)?,
        session_count: row.get(3)?,
        call_count: row.get(4)?,
        input_tokens: row.get(5)?,
        output_tokens: row.get(6)?,
        cache_read_tokens: row.get(7)?,
        cache_write_tokens: row.get(8)?,
        reasoning_tokens: row.get(9)?,
        total_tokens: row.get(10)?,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReportRow {
    pub platform: String,
    pub platform_kind: String,
    pub session_count: i64,
    pub call_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub total_tokens: i64,
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SurfaceReportRow {
    pub platform: String,
    pub platform_kind: String,
    pub surface: String,
    pub session_count: i64,
    pub call_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionStats {
    pub session_count: i64,
    pub duration_secs: i64,
    pub active_days: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectReportRow {
    pub project_path: String,
    pub platform: String,
    pub call_count: i64,
    pub total_tokens: i64,
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DailyTokenRow {
    pub day: String,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DailyPlatformRow {
    pub day: String,
    pub platform: String,
    pub total_tokens: i64,
}

fn map_daily_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DailyTokenRow> {
    Ok(DailyTokenRow {
        day: row.get(0)?,
        total_tokens: row.get(1)?,
    })
}

fn map_daily_platform_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DailyPlatformRow> {
    Ok(DailyPlatformRow {
        day: row.get(0)?,
        platform: row.get(1)?,
        total_tokens: row.get(2)?,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelReportRow {
    pub model: String,
    pub platform: String,
    pub call_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
}

pub fn parse_since(s: &str) -> Result<Option<i64>> {
    let s = s.trim().to_lowercase();
    if s == "all" {
        return Ok(None);
    }
    let now = chrono::Utc::now().timestamp_millis();
    if let Some(days) = s.strip_suffix('d').and_then(|n| n.parse::<i64>().ok()) {
        return Ok(Some(now - days * 86_400_000));
    }
    anyhow::bail!("invalid --since value: use 7d, 30d, or all");
}
