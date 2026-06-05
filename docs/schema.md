# SQLite schema

Default database: `%USERPROFILE%\.config\tokens\tokens.db` (override with `--db`).

## `usage_events`

One row per API call / assistant turn (common denominator across platforms).

| Column | Description |
|--------|-------------|
| `platform` | Registry id, e.g. `claude_code` |
| `platform_kind` | `cli`, `ide`, `hybrid`, `server` |
| `surface` | Optional: `vscode`, `cli`, `desktop`, `agent:main` |
| `session_id` | Session / task / composer id |
| `input_tokens` / `output_tokens` | Core counts |
| `cache_read_tokens` / `cache_write_tokens` | Prompt cache (Claude, Codex) |
| `reasoning_tokens` | Codex reasoning output |
| `usage_unit` | `tokens` or `credits` (Postman) |
| `quality` | `exact`, `estimated`, `credits` |
| `source_path` | Provenance file or `postman:api` |

## Views

- `v_daily_summary` — per day, platform, kind
- `v_by_platform` — totals by platform
- `v_by_project` — totals by project path

See [migrations/001_init.sql](../migrations/001_init.sql).
