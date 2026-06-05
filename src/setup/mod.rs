mod defaults;
mod probe;

use crate::db::TokenStore;
use crate::ingest::{run_scan, ScanOptions};
use crate::paths::user_config::{self, UserPathConfig};
use crate::registry::PlatformRegistry;
use anyhow::{Context, Result};
use defaults::{format_path_list, implemented_with_adapters};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect};
use std::path::PathBuf;

pub fn run_setup(
    store: &TokenStore,
    registry: &PlatformRegistry,
    init: bool,
) -> Result<()> {
    if init {
        return run_init(store, registry);
    }
    run_interactive(store, registry)
}

fn run_init(store: &TokenStore, registry: &PlatformRegistry) -> Result<()> {
    println!("初始化：恢复默认路径并扫描…");
    let implemented = implemented_with_adapters(&registry.platforms);
    let statuses = probe::probe_all(&implemented);
    probe::print_probe_table(&statuses);
    user_config::UserPathConfig::clear_path_overrides(store)?;
    let ids: Vec<String> = statuses
        .iter()
        .filter(|s| s.any_exists)
        .map(|s| s.platform_id.clone())
        .collect();
    let ids = if ids.is_empty() {
        defaults::all_implemented_ids(&registry.platforms)
    } else {
        ids
    };
    let cfg = UserPathConfig::default();
    cfg.save_enabled(store, &ids)?;
    let path_cfg = UserPathConfig::load(store)?;
    user_config::install(path_cfg);
    let result = run_scan(
        store,
        &ScanOptions {
            only_platform: None,
            full_rescan: false,
            include_optional_api: false,
        },
    )?;
    println!(
        "完成：入库 {} 条，跳过 {} 条（解析 {} 个文件）",
        result.events_inserted, result.events_skipped, result.files_scanned
    );
    println!("已启用 {} 个平台。", ids.len());
    Ok(())
}

fn run_interactive(store: &TokenStore, registry: &PlatformRegistry) -> Result<()> {
    let theme = ColorfulTheme::default();
    let implemented = implemented_with_adapters(&registry.platforms);
    if implemented.is_empty() {
        anyhow::bail!("没有可配置的平台适配器");
    }

    let statuses = probe::probe_all(&implemented);

    let current_cfg = UserPathConfig::load(store)?;

    let labels: Vec<String> = statuses
        .iter()
        .map(|s| s.display_name.clone())
        .collect();
    // Pre-check every tool whose default data directory exists on disk.
    let default_flags: Vec<bool> = statuses.iter().map(|s| s.any_exists).collect();

    let selected = MultiSelect::with_theme(&theme)
        .with_prompt("选择要统计 Token 的工具（空格切换，回车确认）")
        .items(&labels)
        .defaults(&default_flags)
        .interact()
        .context("选择平台")?;

    if selected.is_empty() {
        println!("未选择任何平台，已取消。");
        return Ok(());
    }

    let mut enabled_ids = Vec::new();
    for idx in selected {
        let platform = &implemented[idx];
        enabled_ids.push(platform.id.clone());

        let status = &statuses[idx];
        let probed: Vec<PathBuf> = status.paths.iter().map(|p| p.path.clone()).collect();
        let current = current_cfg
            .override_paths(&platform.id)
            .map(|s| s.to_vec())
            .unwrap_or(probed.clone());
        let current_str = format_path_list(&current);

        println!();
        println!("── {} ──", platform.display_name);
        if !status.paths.is_empty() {
            println!("  默认数据位置：");
            for p in &status.paths {
                if p.exists {
                    println!("    √  {}", p.path.display());
                } else {
                    println!("       {}", p.path.display());
                }
            }
        }

        let customize = Confirm::with_theme(&theme)
            .with_prompt("自定义抓取路径？")
            .default(false)
            .interact()
            .context("确认自定义路径")?;

        if customize {
            let input: String = Input::with_theme(&theme)
                .with_prompt("路径（多个用 ; 分隔，留空则用默认）")
                .default(current_str)
                .interact_text()
                .context("输入路径")?;
            let paths: Vec<PathBuf> = user_config::parse_path_list(&input);
            let cfg = UserPathConfig::load(store)?;
            if paths.is_empty() {
                cfg.save_paths(store, &platform.id, &[])?;
                println!("  → 使用默认探测路径");
            } else {
                cfg.save_paths(store, &platform.id, &paths)?;
                println!("  → 已保存 {} 个路径", paths.len());
            }
        } else if current_cfg.override_paths(&platform.id).is_some() {
            store.delete_config(&user_config::paths_key(&platform.id))?;
            println!("  → 已清除自定义路径，使用默认");
        }
    }

    let cfg = UserPathConfig::load(store)?;
    cfg.save_enabled(store, &enabled_ids)?;
    user_config::install(UserPathConfig::load(store)?);

    let scan_now = Confirm::with_theme(&theme)
        .with_prompt("立即扫描入库？")
        .default(true)
        .interact()
        .context("确认扫描")?;

    if scan_now {
        let result = run_scan(
            store,
            &ScanOptions {
                only_platform: None,
                full_rescan: false,
                include_optional_api: false,
            },
        )?;
        println!(
            "扫描完成：入库 {} 条，跳过 {} 条",
            result.events_inserted, result.events_skipped
        );
    } else {
        println!("配置已保存。运行 tokens scan 开始抓取。");
    }

    Ok(())
}
