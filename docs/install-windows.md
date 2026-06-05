# Windows 使用说明

适用于 Windows 10 / 11（x64）。

## 1. 安装

### 方式 A：GitHub Release（推荐）

1. 打开 [Releases](https://github.com/Uuclear/tokens/releases)
2. 下载 `tokens-v*-windows-x86_64.zip`
3. 解压到任意目录，例如 `C:\Tools\tokens\`
4. 将该目录加入系统 **PATH**（设置 → 系统 → 关于 → 高级系统设置 → 环境变量 → Path → 新建）

验证：

```powershell
tokens --help
```

### 方式 B：从源码编译

需要 [Rust](https://rustup.rs/)（`rustup` 默认工具链即可）。

```powershell
git clone https://github.com/Uuclear/tokens.git
cd tokens
cargo build --release
```

可执行文件：`target\release\tokens.exe`

```powershell
# 可选：安装到 cargo bin 目录
cargo install --path .
```

## 2. 首次配置

```powershell
tokens setup
```

按提示多选要统计的工具（空格切换，回车确认）。已有数据目录的平台会自动勾选。

一键初始化（默认路径 + 全平台扫描）：

```powershell
tokens setup --init
```

配置与数据库位置：

| 内容 | 路径 |
|------|------|
| SQLite 数据库 | `%APPDATA%\tokens\tokens.db` |
| 启用平台 / 自定义路径 | 存在数据库 `config` 表 |

## 3. 日常使用

```powershell
tokens scan                  # 增量扫描本地日志
tokens overview --since 7d   # 终端彩色总览
tokens report --since 30d --group model
tokens doctor                # 检查路径与数据库
tokens probe cursor          # 查看 Cursor 数据路径
```

## 4. Web 监控面板

```powershell
tokens serve                 # 后台启动，默认 http://0.0.0.0:5790/
tokens serve --pixel         # 像素 / 墨水屏主题
tokens serve --list-themes   # 列出全部 UI 主题
tokens serve --down          # 停止后台服务
```

开发前端时（仓库根目录）：

```powershell
cargo run -- serve --dev
```

## 5. 常见问题

**`cargo build` 提示无法覆盖 tokens.exe**  
先执行 `tokens serve --down`，或在任务管理器中结束 `tokens.exe`。

**终端乱码 / 方框**  
使用 [Windows Terminal](https://aka.ms/terminal)；必要时 `$env:NO_COLOR=1`。

**杀毒软件拦截**  
Release 二进制未签名，可添加信任或使用源码自行编译。

## 6. 卸载

1. `tokens serve --down`
2. 删除 `%APPDATA%\tokens\` 目录
3. 从 PATH 中移除 `tokens.exe` 所在文件夹
