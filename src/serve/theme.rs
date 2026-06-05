//! Dashboard UI themes for `tokens serve`.

use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiTheme {
    #[default]
    Default,
    /// 8-bit pixel style, e-ink friendly.
    Pixel,
    /// Green phosphor terminal aesthetic.
    Terminal,
    /// High-contrast grayscale for e-ink displays.
    Ink,
    /// Warm light “paper” layout.
    Paper,
}

impl UiTheme {
    pub const ALL: [UiTheme; 5] = [
        UiTheme::Default,
        UiTheme::Pixel,
        UiTheme::Terminal,
        UiTheme::Ink,
        UiTheme::Paper,
    ];

    pub fn cli_flag(self) -> &'static str {
        match self {
            Self::Default => "(默认，无需参数)",
            Self::Pixel => "--pixel",
            Self::Terminal => "--terminal",
            Self::Ink => "--ink",
            Self::Paper => "--paper",
        }
    }

    pub fn print_list() {
        println!("tokens serve 可用 UI 主题：\n");
        for t in Self::ALL {
            println!("  {:24}  {}", t.cli_flag(), t.label());
        }
        println!("\n示例: tokens serve --pixel");
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Pixel => "pixel",
            Self::Terminal => "terminal",
            Self::Ink => "ink",
            Self::Paper => "paper",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "默认（渐变暗色）",
            Self::Pixel => "Pixel 像素 / 墨水屏",
            Self::Terminal => "Terminal 终端",
            Self::Ink => "Ink 水墨灰阶",
            Self::Paper => "Paper 纸本亮色",
        }
    }

    pub fn html(self) -> &'static str {
        match self {
            Self::Default => include_str!("themes/default.html"),
            Self::Pixel => include_str!("themes/pixel.html"),
            Self::Terminal => include_str!("themes/terminal.html"),
            Self::Ink => include_str!("themes/ink.html"),
            Self::Paper => include_str!("themes/paper.html"),
        }
    }

    pub fn resolve(pixel: bool, terminal: bool, ink: bool, paper: bool) -> Result<Self> {
        let mut picked = Vec::new();
        if pixel {
            picked.push(Self::Pixel);
        }
        if terminal {
            picked.push(Self::Terminal);
        }
        if ink {
            picked.push(Self::Ink);
        }
        if paper {
            picked.push(Self::Paper);
        }
        match picked.len() {
            0 => Ok(Self::Default),
            1 => Ok(picked[0]),
            _ => bail!("UI 主题只能选一个：--pixel / --terminal / --ink / --paper"),
        }
    }
}
