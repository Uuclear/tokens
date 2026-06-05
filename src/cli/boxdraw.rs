//! Continuous Unicode box drawing (light / rounded).

pub struct BoxStyle {
    pub tl: &'static str,
    pub tr: &'static str,
    pub bl: &'static str,
    pub br: &'static str,
    pub h: &'static str,
    pub v: &'static str,
    pub lm: &'static str,
    pub rm: &'static str,
}

impl BoxStyle {
    pub const ROUNDED: Self = Self {
        tl: "╭",
        tr: "╮",
        bl: "╰",
        br: "╯",
        h: "─",
        v: "│",
        lm: "├",
        rm: "┤",
    };

    pub const ASCII: Self = Self {
        tl: "+",
        tr: "+",
        bl: "+",
        br: "+",
        h: "-",
        v: "|",
        lm: "+",
        rm: "+",
    };
}

pub struct BoxDrawer {
    pub style: BoxStyle,
    pub width: usize,
}

impl BoxDrawer {
    pub fn new(width: usize, unicode: bool) -> Self {
        Self {
            style: if unicode {
                BoxStyle::ROUNDED
            } else {
                BoxStyle::ASCII
            },
            width: width.max(40),
        }
    }

    pub fn inner_w(&self) -> usize {
        self.width.saturating_sub(2)
    }

    pub fn top(&self, title: Option<&str>) -> String {
        let s = &self.style;
        match title {
            None => format!("{}{}{}", s.tl, s.h.repeat(self.inner_w()), s.tr),
            Some(t) => {
                let label = format!(" {t} ");
                let dash = self.inner_w().saturating_sub(visible_len(&label));
                format!("{}{}{}{}", s.tl, label, s.h.repeat(dash), s.tr)
            }
        }
    }

    pub fn bottom(&self) -> String {
        let s = &self.style;
        format!("{}{}{}", s.bl, s.h.repeat(self.inner_w()), s.br)
    }

    pub fn line(&self, content: &str) -> String {
        let s = &self.style;
        format!("{}{}{}", s.v, pad_vis(content, self.inner_w()), s.v)
    }

    pub fn section(&self, label: &str) -> String {
        let s = &self.style;
        let label = format!(" {label} ");
        let dash = self.inner_w().saturating_sub(visible_len(&label));
        format!("{}{}{}{}", s.lm, label, s.h.repeat(dash), s.rm)
    }

}

pub fn visible_len(s: &str) -> usize {
    let mut vis = 0;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            while chars.next().is_some_and(|x| x != 'm') {}
            continue;
        }
        vis += 1;
    }
    vis
}

pub fn pad_vis(s: &str, width: usize) -> String {
    let vis = visible_len(s);
    if vis >= width {
        truncate_vis(s, width)
    } else {
        format!("{s}{}", " ".repeat(width - vis))
    }
}

pub fn truncate_vis(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut vis = 0;
    let mut esc = false;
    for c in s.chars() {
        if esc {
            out.push(c);
            if c == 'm' {
                esc = false;
            }
            continue;
        }
        if c == '\x1b' {
            esc = true;
            out.push(c);
            continue;
        }
        if vis >= max.saturating_sub(1) {
            out.push('…');
            break;
        }
        out.push(c);
        vis += 1;
    }
    out
}
