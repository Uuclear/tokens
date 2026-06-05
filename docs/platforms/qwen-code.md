# Qwen Code

- **Kind:** CLI
- **Status:** implemented

## Paths

- `%HOME%/.qwen/projects/**/*` (JSON / JSONL under `chats/` etc.)

## Fields

- `usageMetadata` on assistant turns (primary):
  - `promptTokenCount`, `candidatesTokenCount`, `thoughtsTokenCount`, `cachedContentTokenCount`
- Legacy: `usage.inputTokens` / `outputTokens` (CLI `--output-format json`)

## Quality

`exact` when logs contain usage metadata
