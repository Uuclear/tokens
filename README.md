# tokens

Multi-platform AI agent **token usage** statistics CLI (Rust). Reads local session logs from coding agents and IDEs, normalizes into SQLite, and reports by platform / project / model.

**Repository:** https://github.com/Uuclear/tokens

## Downloads (Releases)

预编译二进制见 [GitHub Releases](https://github.com/Uuclear/tokens/releases)：

| 平台 | 文件 | 说明 |
|------|------|------|
| Windows x64 | `tokens-*-windows-x86_64.zip` | 含 `tokens.exe` |
| Linux (Debian/Ubuntu glibc) | `tokens-*-linux-x86_64-gnu.tar.gz` | x86_64 GNU |
| macOS | `tokens-*-macos-universal.tar.gz` | Intel + Apple Silicon |

### 详细安装与使用说明

- [Windows](docs/install-windows.md)
- [macOS](docs/install-macos.md)
- [Linux（Debian/Ubuntu）](docs/install-linux.md)

## Supported platforms

| Platform | Kind | Status |
|----------|------|--------|
| Claude Code | CLI | implemented |
| Codex | CLI | implemented |
| OpenCode | CLI | implemented |
| OpenClaw | CLI | implemented |
| Hermes Agent | CLI | implemented |
| Qwen Code | CLI | implemented |
| Cline | hybrid | implemented |
| Kilo CLI / Kilo IDE | CLI / IDE | implemented |
| Cursor | IDE | implemented (exact + estimate) |
| Cherry Studio | IDE | implemented |
| Chatbox | IDE | implemented |
| Qoder / Qoder CN | IDE | research (no local token log) |
| Postman | hybrid | optional API (credits) |
| Dify | server | optional API |

## Quick start

```bash
tokens setup --init
tokens scan
tokens overview --since 7d
tokens serve
```

## Usage

```bash
tokens list-platforms
tokens probe claude_code
tokens doctor
tokens scan
tokens scan --full
tokens overview --since all
tokens report --since 7d --group surface
tokens report --since all --group platform
tokens report --since 30d --group model --json
tokens config set postman_api_key YOUR_KEY
tokens setup
tokens setup --init
tokens serve
tokens serve --pixel
tokens serve --terminal
tokens serve --ink
tokens serve --paper
tokens serve --list-themes
tokens serve --down
```

### 开发 Web UI（改 HTML/JS 无需整包重编）

在仓库根目录：

```bash
cargo run -- serve --dev --pixel
```

修改 `src/serve/themes/*.html`、`src/serve/dashboard.app.js` 后浏览器刷新即可；Rust 改动仍需 `cargo build`。

## Configuration

`setup` stores enabled platforms in `setup.enabled` and optional overrides in `paths.<platform_id>` (semicolon-separated).

| OS | Config directory |
|----|------------------|
| Windows | `%APPDATA%\tokens\` |
| macOS | `~/Library/Application Support/tokens/` |
| Linux | `~/.config/tokens/` |

`serve` runs a background daemon (default port **5790**), rescans every 5 minutes, and serves bundled offline logos.

### macOS & Linux paths

See [docs/platforms/unix-paths.md](docs/platforms/unix-paths.md). macOS includes IDE/desktop paths; Linux prioritizes CLI + XDG.

```bash
tokens probe cursor
tokens doctor
```

Set `NO_COLOR=1` to disable colors. Set `TOKENS_ASCII=1` for ASCII boxes.

## Build from source

```bash
cargo build --release
```

Refresh logos (optional):

```bash
# Windows
.\scripts\fetch-logos.ps1
# macOS / Linux
./scripts/fetch-logos.sh
```

## Release workflow (maintainers)

```bash
git tag v0.1.0
git push origin v0.1.0
```

Or trigger **Actions → Release → Run workflow** on GitHub.

## Optional API features

```bash
cargo build --features optional_api
tokens scan --api
```

Keys: `postman_api_key`, `dify_api_url`, `dify_api_key`, `cursor_session_token`

## Docs

- [Per-platform adapters](docs/platforms/)
- [Unix paths](docs/platforms/unix-paths.md)

## License

MIT — see [LICENSE](LICENSE)
