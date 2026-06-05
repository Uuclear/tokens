# Codex (OpenAI)

- **Kind:** CLI
- **Status:** implemented

## Paths

- `%USERPROFILE%\.codex\sessions\**\rollout-*.jsonl`
- `%USERPROFILE%\.codex\archived_sessions\*.jsonl`
- Surfaces: `cli` (default), `desktop` (Codex Desktop), `ide` (VS Code extension)
- Override: `CODEX_HOME`

## Fields

JSONL events: `token_usage`, `event_msg` with `payload.type=token_count` → `payload.info.last_token_usage`, legacy `payload.token_count`

- `input_tokens`, `output_tokens`, `cached_input_tokens`, `reasoning_output_tokens`

## Quality

`exact`
