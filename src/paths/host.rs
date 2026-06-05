//! Well-known directories per OS (Windows / macOS / Linux).

use std::env;
use std::path::{Path, PathBuf};

/// User home (`$HOME` / `%USERPROFILE%`).
pub fn user_home() -> PathBuf {
    dirs::home_dir()
        .or_else(|| env::var("HOME").ok().map(PathBuf::from))
        .or_else(|| env::var("USERPROFILE").ok().map(PathBuf::from))
        .unwrap_or_default()
}

/// Roaming app config: `%APPDATA%`, macOS `~/Library/Application Support`, Linux `$XDG_CONFIG_HOME` (~/.config).
pub fn app_config_dir() -> PathBuf {
    env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .or_else(dirs::config_dir)
        .unwrap_or_else(|| user_home().join(".config"))
}

/// Local app data: `%LOCALAPPDATA%`, Linux `$XDG_DATA_HOME` (~/.local/share).
pub fn app_data_local_dir() -> PathBuf {
    env::var("LOCALAPPDATA")
        .ok()
        .map(PathBuf::from)
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| user_home().join(".local").join("share"))
}

/// `$XDG_CONFIG_HOME` or `~/.config` (Linux/macOS CLI tools).
pub fn xdg_config_home() -> PathBuf {
    env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| user_home().join(".config"))
}

/// `$XDG_DATA_HOME` or `~/.local/share`.
pub fn xdg_data_home() -> PathBuf {
    env::var("XDG_DATA_HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| user_home().join(".local").join("share"))
}

pub fn first_existing(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|p| p.exists()).cloned()
}

/// VS Code / VSCodium `User/globalStorage` root.
pub fn vscode_global_storage_root() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        return xdg_config_home()
            .join("Code")
            .join("User")
            .join("globalStorage");
    }
    super::expand_path_template("%APPDATA%/Code/User/globalStorage")
}

/// Cursor IDE `User/globalStorage/state.vscdb`.
pub fn cursor_ide_state_db() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        return xdg_config_home()
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb");
    }
    super::expand_path_template("%APPDATA%/Cursor/User/globalStorage/state.vscdb")
}

/// Cursor agent transcripts under `~/.cursor/projects`.
pub fn cursor_projects_dir() -> PathBuf {
    user_home().join(".cursor").join("projects")
}

/// Cursor CLI state under `~/.cursor`.
pub fn cursor_cli_home() -> PathBuf {
    user_home().join(".cursor")
}

/// Kilo CLI SQLite (XDG / `.local/share`).
pub fn kilo_cli_db_candidates() -> Vec<PathBuf> {
    vec![
        xdg_data_home().join("kilo").join("kilo.db"),
        user_home().join(".local").join("share").join("kilo").join("kilo.db"),
    ]
}

pub fn kilo_cli_db() -> PathBuf {
    first_existing(&kilo_cli_db_candidates())
        .unwrap_or_else(|| xdg_data_home().join("kilo").join("kilo.db"))
}

/// Cherry Studio userData roots (Electron).
pub fn cherry_studio_roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(target_os = "linux")]
    {
        v.push(xdg_config_home().join("Cherry Studio"));
        v.push(xdg_config_home().join("cherry-studio"));
        v.push(user_home().join(".config").join("Cherry Studio"));
        v.push(user_home().join(".config").join("cherry-studio"));
    }
    #[cfg(not(target_os = "linux"))]
    {
        let app = app_config_dir();
        v.push(app.join("Cherry Studio"));
        v.push(app.join("cherry-studio"));
    }
    v
}

/// Chatbox userData root (first existing candidate).
pub fn chatbox_roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(target_os = "linux")]
    {
        v.push(xdg_config_home().join("chatbox"));
        v.push(user_home().join(".config").join("chatbox"));
    }
    #[cfg(not(target_os = "linux"))]
    {
        v.push(app_config_dir().join("chatbox"));
    }
    v
}

pub fn chatbox_root() -> PathBuf {
    first_existing(&chatbox_roots()).unwrap_or_else(|| chatbox_roots()[0].clone())
}

/// OpenCode data directory candidates (XDG + legacy).
pub fn opencode_data_dir_candidates() -> Vec<PathBuf> {
    let mut v = vec![
        xdg_data_home().join("opencode"),
        user_home()
            .join(".local")
            .join("share")
            .join("opencode"),
    ];
    if let Ok(local) = env::var("LOCALAPPDATA") {
        v.push(PathBuf::from(local).join("opencode"));
    }
    let app = app_config_dir();
    v.push(app.join("opencode"));
    let mut unique = Vec::new();
    for p in v {
        if !unique.iter().any(|u| u == &p) {
            unique.push(p);
        }
    }
    unique
}

#[cfg(target_os = "macos")]
pub fn claude_desktop_session_roots() -> Vec<PathBuf> {
    let base = app_config_dir().join("Claude");
    vec![
        base.join("local-agent-mode-sessions").join("projects"),
        base.join("local-agent-mode-sessions"),
    ]
}

#[cfg(not(target_os = "macos"))]
pub fn claude_desktop_session_roots() -> Vec<PathBuf> {
    Vec::new()
}

pub fn path_note_for_doctor(path: &Path) -> Option<String> {
    if path.exists() {
        return None;
    }
    #[cfg(target_os = "linux")]
    if path.to_string_lossy().contains("Application Support") {
        return Some("macOS/Windows IDE path — not used on Linux".into());
    }
    None
}
