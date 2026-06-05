# Cursor

- **Kind:** IDE / Agent CLI
- **Status:** implemented

## Paths

| Surface | Windows | macOS | Linux |
|---------|---------|-------|-------|
| IDE DB | `%APPDATA%\Cursor\...\state.vscdb` | `~/Library/Application Support/Cursor/...` | `~/.config/Cursor/...` |
| Agent | `~\.cursor\projects\` | `~/.cursor/projects/` | same |
| CLI | `~\.cursor\` | `~/.cursor/` | same |

See [unix-paths.md](unix-paths.md).

## Local fields

- `bubbleId:*` → `tokenCount.inputTokens` / `outputTokens` (IDE only, **exact**)
- `composerData:*` → `usageData` (estimated amounts)
- Agent CLI `agent-transcripts/*.jsonl` — **no token fields**; tokens estimates from assistant text (`chars / 4`, **estimated**)

## Dashboard API (exact, recommended for Agent CLI)

Cursor does not expose local token totals for Agent CLI. Use the unofficial dashboard API (same source as [cursor.com/settings](https://cursor.com/settings) usage):

1. Browser DevTools → Application → Cookies → `cursor.com` → copy **`WorkosCursorSessionToken`**
2. `tokens config set cursor_session_token <value>`
3. Build with API support: `cargo build --release --features optional_api,cursor_api`
4. `tokens scan --api`

Note: `~/.config/cursor/auth.json` (`accessToken`) is **not** the dashboard session cookie.

## Quality

| Source | Quality |
|--------|---------|
| IDE `state.vscdb` bubbleId | `exact` |
| Dashboard API | `exact` |
| Agent transcripts | `estimated` |
