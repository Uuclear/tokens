# Cline

- **Kind:** hybrid (VS Code + CLI)
- **Status:** implemented

## Paths

- CLI: `%USERPROFILE%\.cline\data\tasks\<taskId>\`
- VS Code: `%APPDATA%\Code\User\globalStorage\saoudrizwan.claude-dev\tasks\<taskId>\`
- Override: `CLINE_DIR`

## Fields

`ui_messages.json` entries with `say: "api_req_started"` → JSON in `text` with `tokens.input` / `tokens.output`

## Surfaces

- `cli`, `vscode` (stored in `surface` column)

## Quality

`exact`
