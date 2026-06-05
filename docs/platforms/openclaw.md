# OpenClaw

- **Kind:** CLI / Agent
- **Status:** implemented

## Paths

- `%USERPROFILE%\.openclaw\agents\<agentId>\sessions\*.jsonl`
- Index: `sessions.json` (aggregate counters)
- Override: `OPENCLAW_STATE_DIR`

## Fields

Transcript `type: "message"`, `message.role: "assistant"` → `message.usage` (input/output tokens, optional `usage.cost.total`)

## Quality

`exact` (some counters best-effort in index)
