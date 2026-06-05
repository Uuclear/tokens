use std::env;
use std::path::{Path, PathBuf};

pub mod discovery;
pub mod host;
pub mod user_config;

/// Expand `%VAR%`, `~`, and Windows-style placeholders on any OS.
///
/// `%USERPROFILE%` / `%HOME%` → user home; `%APPDATA%` → OS config dir
/// (`~/Library/Application Support` on macOS, `~/.config` on Linux);
/// `%LOCALAPPDATA%` → local data dir (`~/.local/share` on Unix).
pub fn expand_path_template(template: &str) -> PathBuf {
    let mut s = template.to_string();
    let home = dirs::home_dir()
        .or_else(|| env::var("HOME").ok().map(PathBuf::from))
        .or_else(|| env::var("USERPROFILE").ok().map(PathBuf::from))
        .unwrap_or_default();
    let home_s = home.to_string_lossy();

    let appdata = env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .or_else(dirs::config_dir)
        .unwrap_or_else(|| home.join(".config"));
    let appdata_s = appdata.to_string_lossy();

    let localappdata = env::var("LOCALAPPDATA")
        .ok()
        .map(PathBuf::from)
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| home.join(".local").join("share"));
    let local_s = localappdata.to_string_lossy();

    if let Ok(userprofile) = env::var("USERPROFILE") {
        s = s.replace("%USERPROFILE%", &userprofile);
    }
    let xdg_config = host::xdg_config_home();
    let xdg_data = host::xdg_data_home();
    let xdg_config_s = xdg_config.to_string_lossy();
    let xdg_data_s = xdg_data.to_string_lossy();

    s = s.replace("%USERPROFILE%", home_s.as_ref());
    s = s.replace("%HOME%", home_s.as_ref());
    s = s.replace("%APPDATA%", appdata_s.as_ref());
    s = s.replace("%LOCALAPPDATA%", local_s.as_ref());
    s = s.replace("%XDG_CONFIG_HOME%", xdg_config_s.as_ref());
    s = s.replace("%XDG_DATA_HOME%", xdg_data_s.as_ref());

    if s.starts_with('~') {
        s = s.replacen('~', home_s.as_ref(), 1);
    }
    #[cfg(not(windows))]
    {
        s = s.replace('\\', "/");
    }
    PathBuf::from(s)
}

pub fn default_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| env::var("USERPROFILE").map(PathBuf::from).unwrap_or_default())
        .join("tokens")
}

pub fn default_db_path() -> PathBuf {
    default_config_dir().join("tokens.db")
}

pub fn codex_home() -> PathBuf {
    if let Some(p) = user_config::get().override_paths("codex").and_then(|v| v.first()) {
        if p.is_dir() {
            return p.clone();
        }
        if let Some(parent) = p.parent() {
            if parent.file_name().is_some_and(|n| n == "sessions" || n == "archived_sessions") {
                return parent.to_path_buf();
            }
        }
    }
    env::var("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| expand_path_template("%HOME%/.codex"))
}

pub fn claude_config_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(dir) = env::var("CLAUDE_CONFIG_DIR") {
        for part in dir.split(',') {
            let p = PathBuf::from(part.trim());
            if p.join("projects").exists() {
                dirs.push(p);
            } else if p.exists() {
                dirs.push(p);
            }
        }
    }
    let default = expand_path_template("%HOME%/.claude");
    if !dirs.iter().any(|d| d == &default) {
        dirs.push(default);
    }
    dirs
}

pub fn openclaw_state_dir() -> PathBuf {
    first_override_or("openclaw", || {
        env::var("OPENCLAW_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| expand_path_template("%HOME%/.openclaw"))
    })
}

pub fn hermes_home() -> PathBuf {
    first_override_or("hermes", || {
        env::var("HERMES_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| expand_path_template("%HOME%/.hermes"))
    })
}

fn first_override_or(platform: &str, default: impl FnOnce() -> PathBuf) -> PathBuf {
    user_config::get()
        .override_paths(platform)
        .and_then(|v| v.first().cloned())
        .unwrap_or_else(default)
}

pub fn opencode_data_dir() -> PathBuf {
    discovery::opencode_data_dirs()
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| expand_path_template("%XDG_DATA_HOME%/opencode"))
}

pub fn opencode_data_dirs() -> Vec<PathBuf> {
    discovery::opencode_data_dirs()
}

pub fn cline_data_dir() -> PathBuf {
    first_override_or("cline", || {
        env::var("CLINE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| expand_path_template("%HOME%/.cline/data"))
    })
}

pub fn qwen_projects_dir() -> PathBuf {
    first_override_or("qwen_code", || {
        expand_path_template("%HOME%/.qwen/projects")
    })
}

pub fn cursor_global_storage() -> PathBuf {
    host::cursor_ide_state_db()
}

pub fn cursor_projects_dir() -> PathBuf {
    host::cursor_projects_dir()
}

pub fn vscode_global_storage(extension: &str) -> PathBuf {
    host::vscode_global_storage_root()
        .join(extension)
        .join("tasks")
}

pub fn exists_nonempty(path: &Path) -> bool {
    path.exists() && path.metadata().map(|m| m.len() > 0).unwrap_or(false)
}

/// Open foreign SQLite in read-only mode (URI).
pub fn sqlite_readonly_uri(path: &Path) -> String {
    let normalized = path.display().to_string().replace('\\', "/");
    format!("file:{normalized}?mode=ro&immutable=1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_config_uses_xdg_on_linux_paths() {
        let p = expand_path_template("%XDG_CONFIG_HOME%/Cursor/User/globalStorage/state.vscdb");
        let s = p.to_string_lossy();
        assert!(s.contains("Cursor"));
        assert!(!s.contains('\\'));
    }

    #[test]
    fn unix_dot_dirs_use_forward_slashes() {
        for template in [
            "%HOME%/.claude",
            "%HOME%/.codex",
            "%HOME%/.openclaw",
            "%HOME%/.hermes",
            "%HOME%/.cline/data",
            "%HOME%/.qwen/projects",
            "%XDG_DATA_HOME%/opencode",
        ] {
            let p = expand_path_template(template);
            let s = p.to_string_lossy();
            assert!(!s.contains('\\'), "backslash in {template}: {s}");
        }
    }
}
