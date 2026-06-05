# Cursor

- **Kind:** IDE
- **Status:** implemented

## Paths

| Surface | Windows | macOS | Linux |
|---------|---------|-------|-------|
| IDE DB | `%APPDATA%\Cursor\...\state.vscdb` | `~/Library/Application Support/Cursor/...` | `~/.config/Cursor/...` |
| Agent | `~\.cursor\projects\` | `~/.cursor/projects/` | same |
| CLI | `~\.cursor\` | `~/.cursor/` | same |

See [unix-paths.md](unix-paths.md).

## Fields

- `bubbleId:*` → `tokenCount.inputTokens` / `outputTokens`
- `composerData:*` → `usageData` (estimated amounts)
- If `tokenCount` is zero: estimate from `text` length (`chars / 4`)

## Optional API

`tokens config set cursor_session_token <token>` + `tokens scan --api` (feature `cursor_api`)

## Quality

`exact` when `tokenCount` present; else `estimated`
