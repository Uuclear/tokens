# Linux 使用说明（Debian / Ubuntu 等）

适用于 x86_64 的 **glibc** 发行版（Debian 11+、Ubuntu 20.04+、Linux Mint 等）。  
Release 产物：`linux-x86_64-gnu`（`x86_64-unknown-linux-gnu`）。

> Alpine 等 **musl** 系统需从源码编译，见下文。

## 1. 安装

### 方式 A：GitHub Release（推荐）

```bash
cd /tmp
curl -fsSL -LO https://github.com/Uuclear/tokens/releases/latest/download/tokens-v0.1.0-linux-x86_64-gnu.tar.gz
# 将 URL 中的版本号改为你下载的实际版本

tar -xzf tokens-v*-linux-x86_64-gnu.tar.gz
cd tokens-v*-linux-x86_64-gnu
chmod +x tokens
sudo install -m 0755 tokens /usr/local/bin/tokens
```

验证：

```bash
tokens --help
```

### 方式 B：从源码编译

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

git clone https://github.com/Uuclear/tokens.git
cd tokens
cargo build --release
sudo install -m 0755 target/release/tokens /usr/local/bin/tokens
```

### Alpine（musl）

```bash
apk add build-base musl-dev openssl-dev
cargo build --release
```

## 2. Linux 上的数据路径特点

Linux 以 **CLI + XDG** 为主；若安装了桌面版 Cursor / VS Code，也会扫描对应配置目录：

| 类型 | 示例路径 |
|------|----------|
| CLI | `~/.claude`、`~/.codex`、`~/.openclaw` |
| XDG 数据 | `~/.local/share/opencode`、`$XDG_DATA_HOME/kilo/kilo.db` |
| XDG 配置 | `~/.config/Cursor/`、`~/.config/Code/` |
| Cursor Agent | `~/.cursor/projects/` |
| Electron | `~/.config/cherry-studio`、`~/.config/chatbox` |

**不会**使用 macOS 专用的 `~/Library/Application Support`。详见 [unix-paths.md](platforms/unix-paths.md)。

## 3. 首次配置

```bash
tokens setup
tokens setup --init
```

配置目录：`~/.config/tokens/`（`tokens.db` 位于此目录）。

## 4. 日常使用

```bash
tokens scan
tokens overview --since 7d
tokens report --since all --group platform --json
tokens probe opencode
tokens doctor
```

## 5. Web 监控面板

```bash
tokens serve              # 后台 http://0.0.0.0:5790/（监听所有网卡）
tokens serve --ink        # 水墨灰阶主题（适合墨水屏浏览器）
tokens serve --list-themes
tokens serve --down
```

局域网访问示例（假设本机 IP 为 `192.168.1.10`）：

```bash
tokens serve --down
tokens serve
# 浏览器打开 http://192.168.1.10:5790/
```

公网暴露需额外配置路由器端口转发（5790）与防火墙放行；面板无鉴权，请勿直接暴露到公网。

开发：

```bash
cargo run -- serve --dev
```

## 6. systemd 用户服务（可选）

创建 `~/.config/systemd/user/tokens-serve.service`：

```ini
[Unit]
Description=tokens web dashboard
After=network.target

[Service]
ExecStart=/usr/local/bin/tokens serve --foreground
Restart=on-failure

[Install]
WantedBy=default.target
```

```bash
systemctl --user daemon-reload
systemctl --user enable --now tokens-serve
```

注意：默认 `serve` 已自带后台模式；若用 `--foreground` 则由 systemd 管理进程。

## 7. 常见问题

**`cannot execute binary file`**  
架构不匹配（例如在 ARM 板上下载 x86_64 包），请从源码编译。

**`error while loading shared libraries`**  
多为非 glibc 系统，请用源码 `cargo build --release`。

**未找到平台数据**  
运行 `tokens probe <平台>` 确认路径；用 `tokens setup` 手动指定。

## 8. 卸载

```bash
tokens serve --down
rm -rf ~/.config/tokens
sudo rm -f /usr/local/bin/tokens
```
