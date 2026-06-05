use crate::cli::branding::platform_hex;
use crate::db::{parse_since, TokenStore};
use crate::registry::PlatformRegistry;
use crate::serve::logos::{self, local_logo_path};
use crate::serve::theme::UiTheme;
use crate::util::format::{format_duration_secs, format_tokens};
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const APP_JS: &str = include_str!("dashboard.app.js");

#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
    pub registry: Arc<PlatformRegistry>,
    pub ui: UiTheme,
    /// When set, HTML/JS are read from disk on each request (dev hot reload).
    pub dev_assets: Option<PathBuf>,
}

/// Locate `src/serve` for `--dev` (repo root or `TOKENS_DEV_ASSETS`).
pub fn discover_dev_assets_root() -> Option<PathBuf> {
    if let Ok(p) = env::var("TOKENS_DEV_ASSETS") {
        let path = PathBuf::from(p);
        if path.join("dashboard.app.js").exists() {
            return Some(path);
        }
    }
    if let Ok(cwd) = env::current_dir() {
        let candidate = cwd.join("src").join("serve");
        if candidate.join("dashboard.app.js").exists() {
            return Some(candidate);
        }
    }
    None
}

fn theme_html_path(root: &Path, ui: UiTheme) -> PathBuf {
    root.join("themes").join(format!("{}.html", ui.id()))
}

fn no_cache_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    h
}

#[derive(Debug, Deserialize)]
pub struct SinceQuery {
    #[serde(default = "default_since")]
    pub since: String,
}

fn default_since() -> String {
    "7d".into()
}

#[derive(Serialize)]
pub struct DashboardResponse {
    pub since: String,
    pub grand_total: i64,
    pub grand_total_fmt: String,
    pub platform_count: usize,
    pub total_sessions: i64,
    pub total_calls: i64,
    pub total_input_fmt: String,
    pub total_output_fmt: String,
    pub platforms: Vec<PlatformSummary>,
    pub daily_chart: DailyChart,
}

#[derive(Serialize)]
pub struct DailyChart {
    pub labels: Vec<String>,
    pub total: Vec<i64>,
    pub series: Vec<DailySeries>,
}

#[derive(Serialize)]
pub struct DailySeries {
    pub platform: String,
    pub display_name: String,
    pub color: String,
    pub logo_url: String,
    pub values: Vec<i64>,
}

#[derive(Serialize)]
pub struct PlatformSummary {
    pub id: String,
    pub display_name: String,
    pub logo_url: String,
    pub color: String,
    pub total_tokens: String,
    pub total_raw: i64,
    pub input_tokens: String,
    pub output_tokens: String,
    pub share_pct: f64,
    pub sessions: i64,
    pub calls: i64,
    pub active_days: i64,
    pub duration: String,
    pub favorite_model: String,
    pub surfaces: Vec<SurfaceSummary>,
    pub top_models: Vec<ModelChip>,
}

#[derive(Serialize)]
pub struct SurfaceSummary {
    pub surface: String,
    pub total_tokens: String,
    pub total_raw: i64,
    pub input_tokens: String,
    pub output_tokens: String,
    pub calls: i64,
    pub share_pct: f64,
}

#[derive(Serialize)]
pub struct ModelChip {
    pub model: String,
    pub total_tokens: String,
    pub total_raw: i64,
    pub calls: i64,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/api/dashboard", get(api_dashboard))
        .route("/logos/:theme/:id", get(logos::serve_themed_logo))
        .route("/logos/:id", get(logos::serve_logo))
        .with_state(state)
}

async fn index(State(state): State<AppState>) -> impl IntoResponse {
    if let Some(ref root) = state.dev_assets {
        let path = theme_html_path(root, state.ui);
        return match tokio::fs::read_to_string(&path).await {
            Ok(body) => (no_cache_headers(), Html(body)).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("dev: cannot read {}: {e}", path.display()),
            )
                .into_response(),
        };
    }
    Html(state.ui.html()).into_response()
}

async fn app_js(State(state): State<AppState>) -> impl IntoResponse {
    if let Some(ref root) = state.dev_assets {
        let path = root.join("dashboard.app.js");
        return match tokio::fs::read_to_string(&path).await {
            Ok(body) => (
                no_cache_headers(),
                [(header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
                body,
            )
                .into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("dev: cannot read {}: {e}", path.display()),
            )
                .into_response(),
        };
    }
    (
        [(header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
        APP_JS,
    )
        .into_response()
}

async fn api_dashboard(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(q): Query<SinceQuery>,
) -> impl IntoResponse {
    match build_dashboard(&state, &q.since, state.ui) {
        Ok(body) => Json(body).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("{{\"error\":\"{e}\"}}"),
        )
            .into_response(),
    }
}

fn build_daily_chart(
    store: &TokenStore,
    since_ms: Option<i64>,
    names: &HashMap<String, String>,
    active_platforms: &HashSet<String>,
    ui: UiTheme,
) -> anyhow::Result<DailyChart> {
    let daily = store.daily_totals(since_ms)?;
    let by_plat = store.daily_totals_by_platform(since_ms)?;

    let labels: Vec<String> = daily.iter().map(|r| r.day.clone()).collect();
    let total: Vec<i64> = daily.iter().map(|r| r.total_tokens).collect();

    let mut plat_days: HashMap<String, HashMap<String, i64>> = HashMap::new();
    for row in by_plat {
        if !active_platforms.is_empty() && !active_platforms.contains(&row.platform) {
            continue;
        }
        plat_days
            .entry(row.platform.clone())
            .or_default()
            .insert(row.day.clone(), row.total_tokens);
    }

    let mut plat_ids: Vec<_> = plat_days.keys().cloned().collect();
    plat_ids.sort();

    let mut series = Vec::new();
    for pid in plat_ids {
        let day_map = plat_days.get(&pid).unwrap();
        let values: Vec<i64> = labels
            .iter()
            .map(|d| *day_map.get(d).unwrap_or(&0))
            .collect();
        if values.iter().all(|&v| v == 0) {
            continue;
        }
        series.push(DailySeries {
            display_name: names.get(&pid).cloned().unwrap_or_else(|| pid.clone()),
            color: platform_hex(&pid).to_string(),
            logo_url: local_logo_path(&pid, ui),
            platform: pid,
            values,
        });
    }

    Ok(DailyChart {
        labels,
        total,
        series,
    })
}

fn build_dashboard(state: &AppState, since: &str, ui: UiTheme) -> anyhow::Result<DashboardResponse> {
    let store = TokenStore::open(&state.db_path)?;
    let since_ms = parse_since(since)?;
    let surfaces = store.report_by_platform_surface(since_ms)?;
    let names: HashMap<String, String> = state
        .registry
        .platforms
        .iter()
        .map(|p| (p.id.clone(), p.display_name.clone()))
        .collect();

    let mut by_plat: HashMap<String, Vec<_>> = HashMap::new();
    for row in &surfaces {
        by_plat.entry(row.platform.clone()).or_default().push(row);
    }

    let mut plat_rows = Vec::new();
    let mut grand_total = 0i64;
    let mut grand_input = 0i64;
    let mut grand_output = 0i64;
    let mut total_sessions = 0i64;
    let mut total_calls = 0i64;
    let mut plat_ids: Vec<_> = by_plat.keys().cloned().collect();
    plat_ids.sort();

    for id in &plat_ids {
        let rows = by_plat.get(id).unwrap();
        let total: i64 = rows.iter().map(|r| r.total_tokens).sum();
        let input: i64 = rows.iter().map(|r| r.input_tokens).sum();
        let output: i64 = rows.iter().map(|r| r.output_tokens).sum();
        let calls: i64 = rows.iter().map(|r| r.call_count).sum();
        let stats = store.session_stats(since_ms, Some(id))?;
        total_sessions += stats.session_count;
        total_calls += calls;
        grand_total += total;
        grand_input += input;
        grand_output += output;

        plat_rows.push((id.clone(), rows, total, input, output, calls, stats));
    }

    let active_set: HashSet<String> = plat_ids.iter().cloned().collect();
    let daily_chart = build_daily_chart(&store, since_ms, &names, &active_set, ui)?;

    let mut summaries = Vec::new();
    for (id, rows, total, input, output, calls, stats) in plat_rows {
        let share_pct = if grand_total > 0 {
            (total as f64 / grand_total as f64) * 100.0
        } else {
            0.0
        };
        let surface_summaries: Vec<SurfaceSummary> = rows
            .iter()
            .map(|r| {
                let sp = if total > 0 {
                    (r.total_tokens as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                SurfaceSummary {
                    surface: r.surface.clone(),
                    total_tokens: format_tokens(r.total_tokens),
                    total_raw: r.total_tokens,
                    input_tokens: format_tokens(r.input_tokens),
                    output_tokens: format_tokens(r.output_tokens),
                    calls: r.call_count,
                    share_pct: sp,
                }
            })
            .collect();

        let models = store.top_models(since_ms, Some(&id), 3)?;
        let favorite = models
            .first()
            .map(|m| m.model.clone())
            .unwrap_or_else(|| "—".into());
        let top_models: Vec<ModelChip> = models
            .into_iter()
            .map(|m| ModelChip {
                model: m.model,
                total_tokens: format_tokens(m.total_tokens),
                total_raw: m.total_tokens,
                calls: m.call_count,
            })
            .collect();

        let hex = platform_hex(&id).to_string();
        summaries.push(PlatformSummary {
            id: id.clone(),
            display_name: names.get(&id).cloned().unwrap_or_else(|| id.clone()),
            logo_url: local_logo_path(&id, ui),
            color: hex,
            total_tokens: format_tokens(total),
            total_raw: total,
            input_tokens: format_tokens(input),
            output_tokens: format_tokens(output),
            share_pct,
            sessions: stats.session_count,
            calls,
            active_days: stats.active_days,
            duration: format_duration_secs(stats.duration_secs),
            favorite_model: favorite,
            surfaces: surface_summaries,
            top_models,
        });
    }

    Ok(DashboardResponse {
        since: since.to_string(),
        grand_total,
        grand_total_fmt: format_tokens(grand_total),
        platform_count: summaries.len(),
        total_sessions,
        total_calls,
        total_input_fmt: format_tokens(grand_input),
        total_output_fmt: format_tokens(grand_output),
        platforms: summaries,
        daily_chart,
    })
}
