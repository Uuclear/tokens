# Qwen Code

- **Kind:** CLI
- **Status:** implemented

## Paths

- `%USERPROFILE%\.qwen\projects\**\*` (JSON / JSONL session files)

## Fields

- `usage.inputTokens` / `outputTokens` (CLI `--output-format json`)
- Interactive `/stats` for session totals (reference schema)

## Quality

`exact` when logs contain usage; otherwise skipped
