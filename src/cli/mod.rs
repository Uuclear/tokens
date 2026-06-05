mod boxdraw;
pub mod branding;
mod display;

use crate::adapters;
use crate::db::{parse_since, TokenStore};
use crate::ingest::{run_scan, ScanOptions};
use crate::paths::{default_db_path, user_config::{self, UserPathConfig}};
use crate::registry::PlatformRegistry;
use crate::util::format::format_tokens;
use anyhow::Result;
use clap::{Parser, Subcommand};
use display::{Display, DoctorLine, ProbeLine};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "tokens")]
#[command(about = "Multi-platform AI agent token usage statistics CLI")]
pub struct Cli {
    #[arg(long, global = true, help = "Path to SQLite database")]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List registered platforms and implementation status
    ListPlatforms,
    /// Probe local data paths for a platform
    Probe {
        platform: String,
    },
    /// Check database and common paths
    Doctor,
    /// Scan local sources and ingest into SQLite
    Scan {
        #[arg(long, help = "Scan only this platform id")]
        only: Option<String>,
        #[arg(long, help = "Ignore fingerprints and rescan all files")]
        full: bool,
        #[arg(long, help = "Include optional cloud API sources")]
        api: bool,
    },
    /// Aggregate report from ingested data
    Report {
        #[arg(long, default_value = "7d", help = "Time window: 7d, 30d, all")]
        since: String,
        #[arg(
            long,
            default_value = "surface",
            value_parser = ["platform", "surface", "project", "model"]
        )]
        group: String,
        #[arg(long, help = "Filter to one platform id")]
        platform: Option<String>,
        #[arg(long, help = "Output JSON")]
        json: bool,
    },
    /// Rich per-platform dashboard (like /stats overview)
    Overview {
        #[arg(long, default_value = "all", help = "Time window: 7d, 30d, all")]
        since: String,
        #[arg(long, help = "Only this platform")]
        platform: Option<String>,
    },
    /// Get or set optional API configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Interactive platform selection and scan path configuration
    Setup {
        #[arg(long, help = "Reset to default paths, enable all platforms, and scan")]
        init: bool,
    },
    /// Web monitoring dashboard (background daemon on port 5790 by default)
    Serve {
        #[arg(long, default_value_t = 5790, help = "HTTP listen port")]
        port: u16,
        #[arg(long, help = "Stop the background server")]
        down: bool,
        #[arg(long, hide = true, help = "Run server in foreground (used by daemon)")]
        foreground: bool,
        #[arg(long, help = "Pixel / e-ink friendly UI")]
        pixel: bool,
        #[arg(long, help = "Green phosphor terminal UI")]
        terminal: bool,
        #[arg(long, help = "High-contrast grayscale ink UI")]
        ink: bool,
        #[arg(long, help = "Warm paper / light reading UI")]
        paper: bool,
        #[arg(long, help = "List dashboard UI themes and exit")]
        list_themes: bool,
        #[arg(
            long,
            help = "Dev mode: load dashboard HTML/JS from src/serve on each request (requires --foreground)"
        )]
        dev: bool,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    Get { key: String },
    Set { key: String, value: String },
    List,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let db_path = cli.db.unwrap_or_else(default_db_path);
    let registry = PlatformRegistry::load_embedded()?;
    let store = TokenStore::open(&db_path)?;
    store.seed_platforms(&registry)?;
    user_config::install(UserPathConfig::load(&store)?);

    match cli.command {
        Commands::Serve {
            port,
            down,
            foreground,
            pixel,
            terminal,
            ink,
            paper,
            list_themes,
            dev,
        } => {
            if list_themes {
                crate::serve::theme::UiTheme::print_list();
                return Ok(());
            }
            let ui = crate::serve::theme::UiTheme::resolve(pixel, terminal, ink, paper)?;
            let foreground = foreground || dev;
            return crate::serve::run_serve(db_path, port, down, foreground, dev, ui);
        }
        Commands::ListPlatforms => cmd_list_platforms(&registry),
        Commands::Probe { platform } => cmd_probe(&platform, &registry),
        Commands::Doctor => cmd_doctor(&store, &db_path),
        Commands::Scan { only, full, api } => cmd_scan(
            &store,
            ScanOptions {
                only_platform: only,
                full_rescan: full,
                include_optional_api: api,
            },
        ),
        Commands::Report {
            since,
            group,
            platform,
            json,
        } => cmd_report(&store, &since, &group, platform.as_deref(), json),
        Commands::Overview { since, platform } => {
            cmd_overview(&store, &registry, &since, platform.as_deref())
        }
        Commands::Config { command } => cmd_config(&store, command),
        Commands::Setup { init } => crate::setup::run_setup(&store, &registry, init),
    }
}

fn platform_names(registry: &PlatformRegistry) -> HashMap<String, String> {
    registry
        .platforms
        .iter()
        .map(|p| (p.id.clone(), p.display_name.clone()))
        .collect()
}

fn cmd_list_platforms(registry: &PlatformRegistry) -> Result<()> {
    let ui = Display::new();
    let rows: Vec<_> = registry
        .platforms
        .iter()
        .map(|p| (p.id.as_str(), p.kind.as_str(), p.status.as_str(), p.display_name.as_str()))
        .collect();
    ui.print_list_platforms(&rows);
    Ok(())
}

fn cmd_probe(platform: &str, registry: &PlatformRegistry) -> Result<()> {
    let ui = Display::new();
    let adapter = adapters::adapter_by_id(platform)
        .ok_or_else(|| anyhow::anyhow!("unknown platform: {platform}"))?;
    let display_name = registry
        .get(platform)
        .map(|p| p.display_name.as_str())
        .unwrap_or(platform);
    let hits = adapter.probe()?;
    let lines: Vec<ProbeLine> = hits
        .into_iter()
        .map(|h| ProbeLine {
            exists: h.exists,
            path: h.path,
            size: h.size_bytes.map(|s| format_bytes(s)),
            note: h.note,
        })
        .collect();
    ui.print_probe(adapter.id(), display_name, &lines);
    Ok(())
}

fn cmd_doctor(store: &TokenStore, db_path: &std::path::Path) -> Result<()> {
    let ui = Display::new();
    let mut adapter_lines = Vec::new();
    for adapter in adapters::all_adapters() {
        let hits = adapter.probe().unwrap_or_default();
        let found = hits.iter().filter(|h| h.exists).count();
        adapter_lines.push(DoctorLine {
            id: adapter.id().to_string(),
            found,
            total: hits.len(),
            paths: hits
                .iter()
                .filter(|h| h.exists)
                .map(|h| h.path.clone())
                .collect(),
        });
    }
    ui.print_doctor(
        &db_path.display().to_string(),
        db_path.exists(),
        store.event_count()?,
        &adapter_lines,
    );
    Ok(())
}

fn cmd_scan(store: &TokenStore, options: ScanOptions) -> Result<()> {
    let ui = Display::new();
    let result = run_scan(store, &options)?;
    ui.print_scan_summary(
        result.events_inserted,
        result.events_skipped,
        result.files_scanned,
        &result.per_platform,
    );
    Ok(())
}

fn cmd_report(
    store: &TokenStore,
    since: &str,
    group: &str,
    platform: Option<&str>,
    json: bool,
) -> Result<()> {
    let since_ms = parse_since(since)?;
    let ui = Display::new();

    match group {
        "platform" => {
            let mut rows = store.report_by_platform(since_ms)?;
            if let Some(p) = platform {
                rows.retain(|r| r.platform == p);
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else {
                ui.print_report_table(&rows);
            }
        }
        "surface" => {
            let mut rows = store.report_by_platform_surface(since_ms)?;
            if let Some(p) = platform {
                rows.retain(|r| r.platform == p);
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else {
                ui.print_surface_table(&rows);
            }
        }
        "project" => {
            let rows = store.report_by_project(since_ms)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else {
                ui.blank();
                ui.title(&format!("Report by project · {since}"));
                for r in rows {
                    ui.println(format!(
                        "  {:<40} {}  {:>6} calls  {}",
                        truncate_path(&r.project_path, 40),
                        ui.muted(&r.platform),
                        r.call_count,
                        format_tokens(r.total_tokens),
                    ));
                }
                ui.rule();
            }
        }
        "model" => {
            let rows = store.top_models(since_ms, platform, 30)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else {
                ui.print_model_table(&rows);
            }
        }
        _ => anyhow::bail!("unknown group"),
    }
    Ok(())
}

fn cmd_overview(
    store: &TokenStore,
    registry: &PlatformRegistry,
    since: &str,
    platform_filter: Option<&str>,
) -> Result<()> {
    let ui = Display::new();
    let names = platform_names(registry);
    let since_ms = parse_since(since)?;
    let since_label = since;
    let surfaces = store.report_by_platform_surface(since_ms)?;
    let platforms: Vec<String> = surfaces
        .iter()
        .map(|s| s.platform.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let targets: Vec<String> = if let Some(p) = platform_filter {
        vec![p.to_string()]
    } else {
        let mut all = platforms;
        all.sort();
        all
    };

    if targets.is_empty() {
        ui.empty_hint(since_label);
        return Ok(());
    }

    let grand_total: i64 = surfaces
        .iter()
        .filter(|s| targets.iter().any(|t| t == &s.platform))
        .map(|s| s.total_tokens)
        .sum();

    ui.print_overview_header(since_label, grand_total, targets.len());

    let since_7d = parse_since("7d")?;
    let since_30d = parse_since("30d")?;

    for plat in targets {
        let plat_surfaces: Vec<_> = surfaces.iter().filter(|s| s.platform == plat).collect();
        if plat_surfaces.is_empty() {
            continue;
        }
        let total_tokens: i64 = plat_surfaces.iter().map(|s| s.total_tokens).sum();
        let sessions = store.session_stats(since_ms, Some(&plat))?;
        let models = store.top_models(since_ms, Some(&plat), 5)?;

        let favorite = models.first().map(|m| m.model.as_str()).unwrap_or("(none)");
        let tokens_7d: i64 = store
            .report_by_platform_surface(since_7d)?
            .iter()
            .filter(|s| s.platform == plat)
            .map(|s| s.total_tokens)
            .sum();
        let tokens_30d: i64 = store
            .report_by_platform_surface(since_30d)?
            .iter()
            .filter(|s| s.platform == plat)
            .map(|s| s.total_tokens)
            .sum();

        let surface_refs: Vec<_> = plat_surfaces.iter().copied().collect();
        ui.print_overview_platform(
            &plat,
            names.get(&plat).map(String::as_str),
            total_tokens,
            sessions.session_count,
            sessions.active_days,
            sessions.duration_secs,
            favorite,
            tokens_7d,
            tokens_30d,
            &surface_refs,
            &models,
        );
    }

    ui.blank();
    Ok(())
}

fn cmd_config(store: &TokenStore, command: ConfigCommands) -> Result<()> {
    let ui = Display::new();
    match command {
        ConfigCommands::Get { key } => {
            let val = store.get_config(&key)?;
            ui.println(val.unwrap_or_else(|| "(not set)".into()));
        }
        ConfigCommands::Set { key, value } => {
            store.set_config(&key, &value)?;
            ui.println(ui.success("saved"));
        }
        ConfigCommands::List => {
            ui.blank();
            ui.title("Configuration");
            let keys = [
                "postman_api_key",
                "dify_api_url",
                "dify_api_key",
                "cursor_session_token",
            ];
            for k in keys {
                let v = store.get_config(k)?;
                ui.kv(k, v.as_deref().unwrap_or("(not set)"));
            }
            ui.rule();
        }
    }
    Ok(())
}

fn format_bytes(n: u64) -> String {
    format_tokens(n as i64)
}

fn truncate_path(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    if max <= 3 {
        return s.chars().take(max).collect();
    }
    let keep = max - 1;
    let start = s.len().saturating_sub(keep);
    format!("…{}", &s[start..])
}
