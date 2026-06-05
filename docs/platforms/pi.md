# Pi Coding Agent

- **Kind:** CLI (`@earendil-works/pi-coding-agent`)
- **Status:** implemented

## Paths

- `%HOME%/.pi/agent/sessions/**/*.jsonl`
- Override: `PI_AGENT_HOME` (sessions expected under `$PI_AGENT_HOME/sessions`)

## Fields

From `type: "message"` lines → `message.usage`:

- `input` / `output`
- `cacheRead` / `cacheWrite`

## Quality

`exact` when usage fields are non-zero
