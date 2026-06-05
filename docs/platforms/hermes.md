# Hermes Agent

- **Kind:** CLI
- **Status:** implemented

## Paths

- `%USERPROFILE%\.hermes\state.db` (`sessions` table — per-session token totals)
- Override: `HERMES_HOME`

## Fields

SQLite `messages`: `input_tokens`, `output_tokens`, `model`, `session_id`

CLI also exposes `/usage` and status bar (not ingested separately).

## Quality

`exact`
