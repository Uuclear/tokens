mod daemon;
pub mod logos;
pub mod theme;
mod themed_logos;
mod web;

use crate::db::TokenStore;
use crate::ingest::{run_scan, ScanOptions};
use crate::paths::user_config::{self, UserPathConfig};
use crate::registry::PlatformRegistry;
use anyhow::{Context, Result};
use axum::Router;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use theme::UiTheme;
use web::AppState;

pub fn run_serve(
    db_path: PathBuf,
    port: u16,
    down: bool,
    foreground: bool,
    dev: bool,
    ui: UiTheme,
) -> Result<()> {
    if down {
        return daemon::stop();
    }
    if dev && !foreground {
        anyhow::bail!("--dev 需与 --foreground 一起使用（在仓库根目录前台运行）");
    }
    if !foreground {
        return daemon::start_background(port, ui);
    }
    run_foreground_server(db_path, port, ui, dev)
}

fn run_foreground_server(db_path: PathBuf, port: u16, ui: UiTheme, dev: bool) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("tokio runtime")?;

    rt.block_on(async {
        let store = TokenStore::open(&db_path)?;
        let registry = Arc::new(PlatformRegistry::load_embedded()?);
        store.seed_platforms(&registry)?;
        let path_cfg = UserPathConfig::load(&store)?;
        user_config::install(path_cfg);

        let dev_assets: Option<PathBuf> = if dev {
            Some(
                web::discover_dev_assets_root().ok_or_else(|| {
                    anyhow::anyhow!(
                        "未找到 src/serve（请在仓库根目录运行，或设置 TOKENS_DEV_ASSETS=路径）"
                    )
                })?,
            )
        } else {
            None
        };

        let state = AppState {
            db_path: db_path.clone(),
            registry,
            ui,
            dev_assets,
        };

        spawn_scan_loop(db_path.clone());

        let app: Router = web::router(state);
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("bind {addr}"))?;

        println!("tokens 监控 ({}, {}): http://{addr}/", ui.id(), ui.label());
        if dev {
            println!("开发模式：HTML/JS 从磁盘加载，改 src/serve 后浏览器刷新即可");
            println!("（修改 Rust 代码仍需重新 cargo build）");
        }
        println!("按 Ctrl+C 停止");

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .context("http serve")?;

        daemon::cleanup_on_exit();
        Ok::<(), anyhow::Error>(())
    })
}

fn spawn_scan_loop(db_path: PathBuf) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        interval.tick().await;
        loop {
            interval.tick().await;
            let path = db_path.clone();
            let _ = tokio::task::spawn_blocking(move || {
                if let Ok(store) = TokenStore::open(&path) {
                    if let Ok(cfg) = UserPathConfig::load(&store) {
                        user_config::install(cfg);
                    }
                    let _ = run_scan(
                        &store,
                        &ScanOptions {
                            only_platform: None,
                            full_rescan: false,
                            include_optional_api: false,
                        },
                    );
                }
            })
            .await;
        }
    });
}

async fn shutdown_signal() {
    #[cfg(windows)]
    {
        let _ = signal::ctrl_c().await;
    }
    #[cfg(not(windows))]
    {
        let mut term =
            signal::unix::signal(signal::unix::SignalKind::terminate()).expect("SIGTERM");
        tokio::select! {
            _ = signal::ctrl_c() => {},
            _ = term.recv() => {},
        }
    }
}
