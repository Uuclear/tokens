//! Probe default scan paths before interactive setup.

use crate::adapters;
use crate::registry::PlatformEntry;
use crate::setup::defaults::default_paths_for;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PathStatus {
    pub path: PathBuf,
    pub exists: bool,
}

#[derive(Debug, Clone)]
pub struct PlatformPathStatus {
    pub platform_id: String,
    pub display_name: String,
    pub paths: Vec<PathStatus>,
    pub any_exists: bool,
}

pub fn probe_platform(platform: &PlatformEntry) -> PlatformPathStatus {
    let paths: Vec<PathStatus> = if let Some(adapter) = adapters::adapter_by_id(&platform.id) {
        adapter
            .probe()
            .map(|hits| {
                hits.into_iter()
                    .map(|hit| PathStatus {
                        path: PathBuf::from(hit.path),
                        exists: hit.exists,
                    })
                    .collect()
            })
            .unwrap_or_else(|_| probe_registry_paths(platform))
    } else {
        probe_registry_paths(platform)
    };
    let any_exists = paths.iter().any(|p| p.exists);
    PlatformPathStatus {
        platform_id: platform.id.clone(),
        display_name: platform.display_name.clone(),
        paths,
        any_exists,
    }
}

fn probe_registry_paths(platform: &PlatformEntry) -> Vec<PathStatus> {
    default_paths_for(platform)
        .into_iter()
        .map(|path| PathStatus {
            exists: path.exists(),
            path,
        })
        .collect()
}

pub fn probe_all(platforms: &[&PlatformEntry]) -> Vec<PlatformPathStatus> {
    platforms.iter().map(|p| probe_platform(p)).collect()
}

pub fn print_probe_table(statuses: &[PlatformPathStatus]) {
    println!();
    println!("  正在探测本机默认数据目录…");
    println!();
    for s in statuses {
        println!("  {}", s.display_name);
        if s.paths.is_empty() {
            println!("         (未配置默认路径)");
            continue;
        }
        for p in &s.paths {
            if p.exists {
                println!("      √  {}", p.path.display());
            } else {
                println!("         {}", p.path.display());
            }
        }
    }
    println!();
}
