# Kilo CLI

- **Kind:** CLI
- **Status:** implemented

## Paths

- `%USERPROFILE%\.local\share\kilo\kilo.db`
- Alt sessions: `%USERPROFILE%\.kilocode\cli\global\tasks\`

## Fields

SQLite tables (schema varies): JSON cells with `usage` / `tokens`

Built-in `kilo stats` can cross-check totals.

## Quality

`exact` when DB contains usage JSON
