use crate::paths::host;
use std::path::PathBuf;

fn use_overrides(platform: &str, defaults: impl FnOnce() -> Vec<PathBuf>) -> Vec<PathBuf> {
    if let Some(custom) = crate::paths::user_config::get().override_paths(platform) {
        return custom.to_vec();
    }
    defaults()
}

/// All candidate directories that might exist.
pub fn opencode_data_dirs() -> Vec<PathBuf> {
    if let Some(custom) = crate::paths::user_config::get().override_paths("opencode") {
        return custom.to_vec();
    }
    host::opencode_data_dir_candidates()
}

pub fn claude_all_roots() -> Vec<ClaudeRoot> {
    if let Some(custom) = crate::paths::user_config::get().override_paths("claude_code") {
        return custom
            .iter()
            .map(|path| {
                let surface = if path.to_string_lossy().contains("Claude") {
                    "desktop".into()
                } else {
                    "cli".into()
                };
                ClaudeRoot {
                    path: path.clone(),
                    surface,
                    label: "custom".into(),
                }
            })
            .collect();
    }
    let mut roots = Vec::new();
    for dir in crate::paths::claude_config_dirs() {
        let projects = dir.join("projects");
        if projects.exists() {
            roots.push(ClaudeRoot {
                path: projects,
                surface: "cli".into(),
                label: "claude_code_cli".into(),
            });
        }
    }
    for desktop in host::claude_desktop_session_roots() {
        if desktop.exists() {
            let surface = "desktop".into();
            let label = if desktop.ends_with("projects") {
                "claude_desktop".into()
            } else {
                "claude_desktop".into()
            };
            roots.push(ClaudeRoot {
                path: desktop,
                surface,
                label,
            });
        }
    }
    roots
}

#[derive(Debug, Clone)]
pub struct ClaudeRoot {
    pub path: PathBuf,
    pub surface: String,
    pub label: String,
}

pub fn codex_roots() -> Vec<PathBuf> {
    use_overrides("codex", || {
        let home = host::user_home();
        vec![
            home.join(".codex").join("sessions"),
            home.join(".codex").join("archived_sessions"),
        ]
    })
}

pub fn cursor_roots() -> Vec<CursorRoot> {
    if let Some(custom) = crate::paths::user_config::get().override_paths("cursor") {
        return custom
            .iter()
            .map(|path| {
                let kind = if path.extension().is_some_and(|e| e == "vscdb") {
                    CursorRootKind::Vscdb
                } else if path
                    .file_name()
                    .is_some_and(|n| n.eq_ignore_ascii_case("projects"))
                {
                    CursorRootKind::AgentTranscripts
                } else {
                    CursorRootKind::CliStore
                };
                let surface = match kind {
                    CursorRootKind::Vscdb => "ide",
                    CursorRootKind::AgentTranscripts => "agent",
                    CursorRootKind::CliStore => "cli",
                };
                CursorRoot {
                    path: path.clone(),
                    surface: surface.into(),
                    kind,
                }
            })
            .collect();
    }
    let mut roots = Vec::new();
    let ide_db = host::cursor_ide_state_db();
    if ide_db.exists() {
        roots.push(CursorRoot {
            path: ide_db,
            surface: "ide".into(),
            kind: CursorRootKind::Vscdb,
        });
    }
    let agent_projects = host::cursor_projects_dir();
    if agent_projects.exists() {
        roots.push(CursorRoot {
            path: agent_projects,
            surface: "agent".into(),
            kind: CursorRootKind::AgentTranscripts,
        });
    }
    let cursor_home = host::cursor_cli_home();
    if cursor_home.exists() {
        roots.push(CursorRoot {
            path: cursor_home,
            surface: "cli".into(),
            kind: CursorRootKind::CliStore,
        });
    }
    roots
}

#[derive(Debug, Clone)]
pub struct CursorRoot {
    pub path: PathBuf,
    pub surface: String,
    pub kind: CursorRootKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorRootKind {
    Vscdb,
    AgentTranscripts,
    CliStore,
}

pub fn first_existing(paths: &[PathBuf]) -> Option<PathBuf> {
    host::first_existing(paths)
}
