# OpenCode

- **Kind:** CLI
- **Status:** implemented

## Paths

- v1.2+: `%USERPROFILE%\.local\share\opencode\opencode.db`
- Legacy: `%USERPROFILE%\.local\share\opencode\storage\message\**\*.json`

## Fields

SQLite messages: `tokens.input`, `tokens.output`, `cost`, `providerID`, `modelID`

Legacy JSON: same `tokens` object per message file.

## Quality

`exact`
