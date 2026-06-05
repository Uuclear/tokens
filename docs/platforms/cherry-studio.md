# Cherry Studio

- **Kind:** IDE (Electron)
- **Status:** implemented

## Paths

| OS | Location |
|----|----------|
| Windows | `%APPDATA%\Cherry Studio\` or `cherry-studio\` |
| macOS | `~/Library/Application Support/Cherry Studio` (or `cherry-studio`) |
| Linux | `~/.config/Cherry Studio` / `~/.config/cherry-studio` |

Trace / span JSON under userData. See [unix-paths.md](unix-paths.md).

## Fields

Recursive search for `input_tokens`, `output_tokens`, `provider`, `model`, `topicId`

Telemetry to cherry-ai.com is **not** required; local trace only.

## Quality

`exact` when local trace files contain counts
