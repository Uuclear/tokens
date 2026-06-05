# Chatbox

- **Kind:** IDE / desktop client
- **Status:** implemented

## Paths

| OS | Location |
|----|----------|
| Windows / macOS | `%APPDATA%\chatbox\` / `~/Library/Application Support/chatbox` |
| Linux | `~/.config/chatbox` |

Includes `chatbox-blobs`. See [unix-paths.md](unix-paths.md).

## Fields

Message objects with `prompt_tokens` / `completion_tokens` (or `input_tokens` / `output_tokens`)

Use `tokens probe chatbox` to list detected paths.

## Quality

`exact` when fields present in local JSON
