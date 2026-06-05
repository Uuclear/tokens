# macOS 使用说明

适用于 macOS 12+（Intel 与 Apple Silicon；Release 提供 **Universal** 二进制）。

## 1. 安装

### 方式 A：GitHub Release（推荐）

1. 打开 [Releases](https://github.com/Uuclear/tokens/releases)
2. 下载 `tokens-v*-macos-universal.tar.gz`
3. 解压并安装到 PATH：

```bash
tar -xzf tokens-v*-macos-universal.tar.gz
cd tokens-v*-macos-universal
chmod +x tokens
sudo mv tokens /usr/local/bin/tokens
# 或使用 ~/.local/bin（需将该目录加入 PATH）
mkdir -p ~/.local/bin && mv tokens ~/.local/bin/
```

验证：

```bash
tokens --help
```

若提示「无法验证开发者」：系统设置 → 隐私与安全性 → 仍要打开；或 `xattr -dr com.apple.quarantine /usr/local/bin/tokens`。

### 方式 B：从源码编译

```bash
xcode-select --install   # 若尚未安装命令行工具
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
git clone https://github.com/Uuclear/tokens.git
cd tokens
cargo build --release
./target/release/tokens --help
```

## 2. macOS 上的数据路径特点

macOS 同时支持 **CLI 工具** 与 **IDE / 桌面应用**：

| 类型 | 示例路径 |
|------|----------|
| CLI | `~/.claude/projects`、`~/.codex/sessions` |
| Cursor IDE | `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb` |
| Cursor Agent | `~/.cursor/projects/` |
| VS Code 插件 (Cline/Kilo) | `~/Library/Application Support/Code/User/globalStorage/...` |
| Claude Desktop | `~/Library/Application Support/Claude/local-agent-mode-sessions` |
| Electron (Cherry Studio / Chatbox) | `~/Library/Application Support/...` |

完整列表见 [unix-paths.md](platforms/unix-paths.md)。

## 3. 首次配置

```bash
tokens setup
tokens setup --init    # 可选：重置路径并扫描
```

配置目录：`~/Library/Application Support/tokens/`（含 `tokens.db`）。

## 4. 日常使用

```bash
tokens scan
tokens overview --since all
tokens report --since 7d --group surface
tokens probe claude_code
tokens doctor
```

## 5. Web 监控面板

```bash
tokens serve              # 后台 http://0.0.0.0:5790/（监听所有网卡）
tokens serve --paper      # 暖色纸张主题
tokens serve --list-themes
tokens serve --down
```

局域网访问示例（假设本机 IP 为 `192.168.1.10`）：

```bash
tokens serve --down
tokens serve
# 浏览器打开 http://192.168.1.10:5790/
```

面板无鉴权，请勿直接暴露到公网；若需远程访问请自行配置防火墙或反向代理。

开发模式（在克隆的仓库根目录）：

```bash
cargo run -- serve --dev --pixel
```

### launchd 用户服务（可选）

创建 `~/Library/LaunchAgents/com.tokens.serve.plist`：

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.tokens.serve</string>
  <key>ProgramArguments</key>
  <array>
    <string>/usr/local/bin/tokens</string>
    <string>serve</string>
    <string>--foreground</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
</dict>
</plist>
```

加载：

```bash
launchctl load ~/Library/LaunchAgents/com.tokens.serve.plist
launchctl unload ~/Library/LaunchAgents/com.tokens.serve.plist   # 停止
```

注意：默认 `serve` 已自带后台模式；若用 `--foreground` 则由 launchd 管理进程。

## 6. 常见问题

**权限被拒绝**  
确保二进制可执行：`chmod +x tokens`。

**Gatekeeper**  
见上文 `xattr` 或系统设置中允许。

**仅 CLI、未装 Cursor IDE**  
`tokens probe cursor` 可能只显示 `~/.cursor` CLI 路径，属正常。

## 7. 卸载

```bash
tokens serve --down
rm -rf ~/Library/Application\ Support/tokens
sudo rm -f /usr/local/bin/tokens   # 或 ~/.local/bin/tokens
```
