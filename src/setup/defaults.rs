//! Default scan paths from registry templates and live probe.

use crate::adapters;
use crate::paths::expand_path_template;
use crate::registry::PlatformEntry;
use std::path::PathBuf;

pub fn default_paths_for(platform: &PlatformEntry) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(ref pp) = platform.paths {
        for template in pp.templates_for_host() {
            let expanded = expand_path_template(strip_glob(&template));
            if !paths.iter().any(|x| x == &expanded) {
                paths.push(expanded);
            }
        }
    }
    paths
}

pub fn format_path_list(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(";")
}

fn strip_glob(template: &str) -> &str {
    template
        .split(['*', '?'])
        .next()
        .unwrap_or(template)
        .trim_end_matches(['/', '\\'])
}

pub fn implemented_with_adapters<'a>(
    platforms: &'a [PlatformEntry],
) -> Vec<&'a PlatformEntry> {
    platforms
        .iter()
        .filter(|p| {
            (p.status == "implemented" || p.status == "optional_api")
                && adapters::adapter_by_id(&p.id).is_some()
        })
        .collect()
}

pub fn all_implemented_ids(platforms: &[PlatformEntry]) -> Vec<String> {
    implemented_with_adapters(platforms)
        .into_iter()
        .map(|p| p.id.clone())
        .collect()
}
