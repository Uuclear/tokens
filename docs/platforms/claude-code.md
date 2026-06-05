# Claude Code

- **Kind:** CLI
- **Status:** implemented

## Paths (Windows)

- CLI: `%USERPROFILE%\.claude\projects\**\*.jsonl` (surface=`cli`)
- Desktop: `%APPDATA%\Claude\local-agent-mode-sessions\` and `...\projects\` (surface=`desktop`)
- Override: `CLAUDE_CONFIG_DIR` (comma-separated roots with `projects/`)

## Fields

From `type: "assistant"` lines → `message.usage`:

- `input_tokens`, `output_tokens`
- `cache_read_input_tokens`, `cache_creation_input_tokens`
- Dedup: `requestId`

## Quality

`exact`
