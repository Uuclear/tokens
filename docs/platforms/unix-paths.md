# macOS & Linux path conventions

`tokens` picks paths from `registry/platforms.yaml` using the **`macos`** or **`linux`** section on each OS (never mixing Windows IDE paths blindly).

## Placeholders

| Placeholder | Windows | macOS | Linux |
|-------------|---------|-------|-------|
| `%HOME%` | — | `~` | `~` |
| `%USERPROFILE%` | profile dir | same as home | same as home |
| `%APPDATA%` | `%APPDATA%` | `~/Library/Application Support` | `~/.config` |
| `%LOCALAPPDATA%` | local app data | `~/Library/Application Support` (fallback) | `~/.local/share` |
| `%XDG_CONFIG_HOME%` | `~/.config` | `~/.config` | `$XDG_CONFIG_HOME` or `~/.config` |
| `%XDG_DATA_HOME%` | `~/.local/share` | `~/.local/share` | `$XDG_DATA_HOME` or `~/.local/share` |

Config DB: `dirs::config_dir()/tokens/tokens.db` (e.g. `~/.config/tokens` on Linux, `~/Library/Application Support/tokens` on macOS).

## Design: macOS vs Linux

| Category | macOS | Linux |
|----------|-------|-------|
| **CLI agents** | `~/.claude`, `~/.codex`, `~/.openclaw`, … | Same under `$HOME` |
| **XDG CLI data** | `~/.local/share/opencode`, `$XDG_DATA_HOME/kilo` | Primary for OpenCode, Kilo CLI |
| **IDE (Cursor)** | `~/Library/Application Support/Cursor/...` + `~/.cursor/projects` | `~/.config/Cursor/...` + `~/.cursor/projects` |
| **VS Code extensions** (Cline, Kilo IDE) | `~/Library/Application Support/Code/User/globalStorage/...` | `~/.config/Code/User/globalStorage/...` |
| **Electron desktops** (Cherry Studio, Chatbox) | `~/Library/Application Support/...` | `~/.config/cherry-studio`, `~/.config/chatbox` |
| **Claude Desktop** | `~/Library/Application Support/Claude/local-agent-mode-sessions` | *not used* (no desktop app) |

Linux installs **do not** probe macOS-only `Application Support` trees. macOS includes **Claude Desktop** session paths in addition to Claude Code CLI.

## Verify on your machine

```bash
tokens probe cursor
tokens probe claude_code
tokens doctor
tokens setup    # auto-selects platforms with existing paths
```

Override any path with `tokens setup` or `tokens config` / `paths.<platform_id>` (semicolon-separated).

## `tokens serve` on Unix

- Foreground: `tokens serve --foreground`
- Background: daemon uses `process_group` + `SIGTERM`; same as Linux/macOS standard practice
- Stop: `tokens serve --down`
