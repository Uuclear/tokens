//! Per-platform colored icon + ANSI brand color.

pub struct PlatformBrand {
    pub badge: &'static str,
    pub ansi: &'static str,
}

pub fn platform_brand(id: &str) -> PlatformBrand {
    match id {
        "claude_code" => PlatformBrand {
            badge: "◆",
            ansi: "\x1b[38;5;208m",
        },
        "codex" => PlatformBrand {
            badge: "◎",
            ansi: "\x1b[38;5;120m",
        },
        "cursor" => PlatformBrand {
            badge: "▸",
            ansi: "\x1b[38;5;39m",
        },
        "opencode" => PlatformBrand {
            badge: "◈",
            ansi: "\x1b[38;5;141m",
        },
        "openclaw" => PlatformBrand {
            badge: "◆",
            ansi: "\x1b[38;5;203m",
        },
        "hermes" => PlatformBrand {
            badge: "⚡",
            ansi: "\x1b[38;5;226m",
        },
        "qwen_code" => PlatformBrand {
            badge: "◆",
            ansi: "\x1b[38;5;33m",
        },
        "pi" => PlatformBrand {
            badge: "π",
            ansi: "\x1b[38;5;177m",
        },
        "cline" => PlatformBrand {
            badge: "◆",
            ansi: "\x1b[38;5;51m",
        },
        "kilo_cli" | "kilo_ide" => PlatformBrand {
            badge: "◆",
            ansi: "\x1b[38;5;213m",
        },
        "cherry_studio" => PlatformBrand {
            badge: "♥",
            ansi: "\x1b[38;5;196m",
        },
        "chatbox" => PlatformBrand {
            badge: "◆",
            ansi: "\x1b[38;5;45m",
        },
        "qoder" | "qoder_cn" => PlatformBrand {
            badge: "◆",
            ansi: "\x1b[38;5;99m",
        },
        "postman" => PlatformBrand {
            badge: "◆",
            ansi: "\x1b[38;5;208m",
        },
        "dify" => PlatformBrand {
            badge: "◆",
            ansi: "\x1b[38;5;27m",
        },
        _ => PlatformBrand {
            badge: "●",
            ansi: "\x1b[90m",
        },
    }
}

/// Web dashboard accent color (hex).
pub fn platform_hex(id: &str) -> &'static str {
    match id {
        "claude_code" => "#f97316",
        "codex" => "#84cc16",
        "cursor" => "#38bdf8",
        "opencode" => "#a78bfa",
        "openclaw" => "#fb7185",
        "hermes" => "#facc15",
        "qwen_code" => "#60a5fa",
        "pi" => "#c084fc",
        "cline" => "#22d3ee",
        "kilo_cli" | "kilo_ide" => "#e879f9",
        "cherry_studio" => "#f43f5e",
        "chatbox" => "#2dd4bf",
        "qoder" | "qoder_cn" => "#818cf8",
        "postman" => "#fb923c",
        "dify" => "#3b82f6",
        _ => "#94a3b8",
    }
}
