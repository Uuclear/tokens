use tokens::paths::{expand_path_template, host};
use tokens::registry::PlatformRegistry;

#[test]
fn registry_has_macos_and_linux_paths_for_implemented_cli() {
    let reg = PlatformRegistry::load_embedded().unwrap();
    for id in [
        "claude_code",
        "codex",
        "opencode",
        "cursor",
        "cline",
        "kilo_cli",
    ] {
        let p = reg.get(id).unwrap();
        let paths = p.paths.as_ref().unwrap();
        assert!(
            paths.macos.as_ref().is_some_and(|v| !v.is_empty()),
            "{id} missing macos paths"
        );
        assert!(
            paths.linux.as_ref().is_some_and(|v| !v.is_empty()),
            "{id} missing linux paths"
        );
    }
}

#[test]
fn expand_xdg_placeholders() {
    let cfg = expand_path_template("%XDG_CONFIG_HOME%/Cursor/User/globalStorage/state.vscdb");
    assert!(cfg.to_string_lossy().contains("Cursor"));
    let data = expand_path_template("%XDG_DATA_HOME%/opencode/opencode.db");
    assert!(data.to_string_lossy().contains("opencode"));
}

#[test]
fn expand_home_unix_style() {
    let p = expand_path_template("%HOME%/.codex/sessions");
    let home = host::user_home();
    assert!(p.starts_with(&home));
}

#[test]
fn linux_cursor_uses_config_not_application_support() {
    let reg = PlatformRegistry::load_embedded().unwrap();
    let cursor = reg.get("cursor").unwrap();
    let linux = cursor.paths.as_ref().unwrap().linux.as_ref().unwrap();
    let joined = linux.join(" ");
    assert!(joined.contains("XDG_CONFIG_HOME") || joined.contains(".config"));
    assert!(!joined.contains("Application Support"));
}
