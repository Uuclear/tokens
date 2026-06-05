//! Terminal presentation helpers (ANSI + continuous box drawing).

use crate::cli::boxdraw::{truncate_vis, visible_len, BoxDrawer, pad_vis};
use crate::cli::branding::{platform_brand, PlatformBrand};
use crate::db::{ModelReportRow, ReportRow, SurfaceReportRow};
use crate::util::format::{format_duration_secs, format_tokens};
use std::io::{self, IsTerminal, Write};

const DEFAULT_WIDTH: usize = 80;
pub struct Display {
    color: bool,
    unicode: bool,
    width: usize,
}

impl Display {
    pub fn new() -> Self {
        let tty = io::stdout().is_terminal();
        let color = tty && std::env::var("NO_COLOR").is_err();
        let ascii = std::env::var("TOKENS_ASCII").is_ok();
        Self {
            color,
            // Continuous Unicode boxes by default (set TOKENS_ASCII=1 to disable).
            unicode: !ascii,
            width: DEFAULT_WIDTH,
        }
    }

    pub fn println(&self, line: impl AsRef<str>) {
        let _ = writeln!(io::stdout(), "{}", line.as_ref());
    }

    pub fn blank(&self) {
        self.println("");
    }

    fn boxer(&self) -> BoxDrawer {
        BoxDrawer::new(self.width, self.unicode)
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("{code}{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    fn bold(&self, s: &str) -> String {
        self.paint("\x1b[1m", s)
    }

    fn dim(&self, s: &str) -> String {
        self.paint("\x1b[2m", s)
    }

    fn cyan(&self, s: &str) -> String {
        self.paint("\x1b[36m", s)
    }

    fn green(&self, s: &str) -> String {
        self.paint("\x1b[32m", s)
    }

    fn yellow(&self, s: &str) -> String {
        self.paint("\x1b[33m", s)
    }

    fn red(&self, s: &str) -> String {
        self.paint("\x1b[31m", s)
    }

    fn magenta(&self, s: &str) -> String {
        self.paint("\x1b[35m", s)
    }

    pub fn platform_label(&self, plat: &str, display_name: &str) -> String {
        let brand = platform_brand(plat);
        if self.color {
            format!(
                "{} {}",
                self.paint(brand.ansi, brand.badge),
                self.bold(display_name)
            )
        } else {
            format!("[{}] {}", brand.badge, display_name)
        }
    }

    pub fn title(&self, text: &str) {
        let b = self.boxer();
        self.println(self.cyan(&b.top(Some(text))));
    }

    pub fn muted(&self, s: &str) -> String {
        self.dim(s)
    }

    pub fn rule(&self) {
        let b = self.boxer();
        self.println(self.dim(&b.line("")));
    }

    pub fn kv(&self, key: &str, value: impl AsRef<str>) {
        let b = self.boxer();
        self.println(b.line(&format!(
            "  {:<16} {}",
            self.dim(&format!("{key}:")),
            value.as_ref()
        )));
    }

    fn stat_line(&self, b: &BoxDrawer, key: &str, value: impl AsRef<str>) -> String {
        b.line(&format!(
            "   {:<14} {}",
            self.dim(&format!("{key}:")),
            value.as_ref()
        ))
    }

    pub fn token_bar_str(
        &self,
        part: i64,
        total: i64,
        width: usize,
        brand: Option<&PlatformBrand>,
    ) -> String {
        if total <= 0 || part <= 0 {
            let empty = if self.unicode {
                "\u{2591}".repeat(width)
            } else {
                ".".repeat(width)
            };
            return self.dim(&empty);
        }
        let filled = ((part as f64 / total as f64) * width as f64).round() as usize;
        let filled = filled.clamp(0, width);
        let empty = width.saturating_sub(filled);
        let full = if self.unicode {
            "\u{2588}".repeat(filled) + &"\u{2591}".repeat(empty)
        } else {
            "#".repeat(filled) + &".".repeat(empty)
        };
        if self.color {
            if let Some(br) = brand {
                self.paint(br.ansi, &full)
            } else {
                self.cyan(&full)
            }
        } else {
            full
        }
    }

    pub fn print_scan_summary(
        &self,
        inserted: i64,
        skipped: i64,
        parsed: i64,
        per_platform: &[(String, i64, i64)],
    ) {
        let b = self.boxer();
        self.blank();
        self.println(self.cyan(&b.top(Some("Scan complete"))));
        self.println(b.line(&format!(
            " {} {}",
            self.dim("Inserted:"),
            self.green(&inserted.to_string())
        )));
        self.println(b.line(&format!(
            " {} {}",
            self.dim("Skipped:"),
            self.dim(&skipped.to_string())
        )));
        self.println(b.line(&format!(" {} {parsed}", self.dim("Parsed:"))));
        if !per_platform.is_empty() {
            self.println(b.section("Platforms"));
            for (plat, ins, skip) in per_platform {
                if *ins == 0 && *skip == 0 {
                    continue;
                }
                let label = self.platform_label(plat, plat);
                let ins_s = if *ins > 0 {
                    self.green(&format!("+{ins}"))
                } else {
                    self.dim("0").to_string()
                };
                self.println(b.line(&format!(
                    " {label}  {ins_s}  {} skipped",
                    self.dim(&skip.to_string())
                )));
            }
        }
        self.println(self.dim(&b.bottom()));
        self.blank();
        self.println(self.dim(
            "Tip: tokens overview --since all  ·  tokens report --group surface",
        ));
    }

    pub fn print_overview_header(
        &self,
        since_label: &str,
        grand_total: i64,
        platform_count: usize,
    ) {
        let b = self.boxer();
        self.blank();
        self.println(self.cyan(&b.top(Some(&format!("Token usage · {since_label}")))));
        if platform_count > 1 {
            self.println(b.line(&format!(
                " {}  ·  {} platforms",
                self.magenta(&format_tokens(grand_total)),
                platform_count
            )));
        }
        self.rule();
    }

    pub fn print_overview_platform(
        &self,
        plat: &str,
        display_name: Option<&str>,
        total_tokens: i64,
        session_count: i64,
        active_days: i64,
        duration_secs: i64,
        favorite: &str,
        tokens_7d: i64,
        tokens_30d: i64,
        surfaces: &[&SurfaceReportRow],
        models: &[ModelReportRow],
    ) {
        let brand = platform_brand(plat);
        let b = self.boxer();
        let name = display_name.filter(|n| !n.is_empty()).unwrap_or(plat);
        let header = self.platform_label(plat, name);

        self.blank();
        self.println(self.cyan(&b.top(None)));
        self.println(b.line(&format!(" {header}")));
        self.println(self.stat_line(&b, "Total", self.magenta(&format_tokens(total_tokens))));
        self.println(self.stat_line(&b, "Sessions", session_count.to_string()));
        self.println(self.stat_line(&b, "Active days", active_days.to_string()));
        self.println(
            self.stat_line(&b, "Time span", format_duration_secs(duration_secs)),
        );
        self.println(self.stat_line(&b, "Favorite", self.cyan(favorite)));
        self.println(self.stat_line(
            &b,
            "Recent",
            format!(
                "{} {}  ·  {} {}",
                self.dim("7d"),
                self.bold(&format_tokens(tokens_7d)),
                self.dim("30d"),
                self.bold(&format_tokens(tokens_30d)),
            ),
        ));

        if !surfaces.is_empty() {
            self.println(b.section("Surfaces"));
            let bar_w = 16;
            for s in surfaces {
                let bar = self.token_bar_str(s.total_tokens, total_tokens, bar_w, Some(&brand));
                self.println(b.line(&format!(
                    "   {:<8} {}  {:>4} sess  {:>6} calls  {}",
                    s.surface,
                    bar,
                    s.session_count,
                    s.call_count,
                    self.bold(&format_tokens(s.total_tokens)),
                )));
                self.println(b.line(&format!(
                    "            {:>8} in  {:>8} out  {:>8} cache",
                    format_tokens(s.input_tokens),
                    format_tokens(s.output_tokens),
                    format_tokens(s.cache_read_tokens),
                )));
            }
        }

        if !models.is_empty() {
            self.println(b.section("Top models"));
            let max_t = models.first().map(|m| m.total_tokens).unwrap_or(1).max(1);
            for (i, m) in models.iter().enumerate() {
                let bar = self.token_bar_str(m.total_tokens, max_t, 10, Some(&brand));
                self.println(b.line(&format!(
                    "   {:>2}. {:<22} {}  {:>5} calls  {}",
                    i + 1,
                    truncate_vis(&m.model, 22),
                    bar,
                    m.call_count,
                    format_tokens(m.total_tokens),
                )));
            }
        }

        self.println(self.dim(&b.bottom()));
    }

    pub fn print_report_table(&self, rows: &[ReportRow]) {
        if rows.is_empty() {
            self.println(self.dim("  (no data)"));
            return;
        }
        self.blank();
        self.title("Report by platform");
        let headers = ["PLATFORM", "KIND", "SESS", "CALLS", "IN", "OUT", "CACHE", "TOTAL"];
        self.print_table_header(&headers);
        for r in rows {
            let name = self.platform_label(&r.platform, &r.platform);
            self.print_table_row(&[
                &name,
                r.platform_kind.as_str(),
                &r.session_count.to_string(),
                &r.call_count.to_string(),
                &format_tokens(r.input_tokens),
                &format_tokens(r.output_tokens),
                &format_tokens(r.cache_read_tokens),
                &format_tokens(r.total_tokens),
            ]);
        }
        self.rule();
    }

    pub fn print_surface_table(&self, rows: &[SurfaceReportRow]) {
        if rows.is_empty() {
            self.println(self.dim("  (no data)"));
            return;
        }
        self.blank();
        self.title("Report by surface");
        let headers = [
            "PLATFORM", "KIND", "SURFACE", "SESS", "CALLS", "IN", "OUT", "CACHE", "TOTAL",
        ];
        self.print_table_header(&headers);
        for r in rows {
            let name = self.platform_label(&r.platform, &r.platform);
            self.print_table_row(&[
                &name,
                r.platform_kind.as_str(),
                r.surface.as_str(),
                &r.session_count.to_string(),
                &r.call_count.to_string(),
                &format_tokens(r.input_tokens),
                &format_tokens(r.output_tokens),
                &format_tokens(r.cache_read_tokens),
                &format_tokens(r.total_tokens),
            ]);
        }
        self.rule();
    }

    pub fn print_model_table(&self, rows: &[ModelReportRow]) {
        if rows.is_empty() {
            self.println(self.dim("  (no data)"));
            return;
        }
        self.blank();
        self.title("Report by model");
        let headers = ["MODEL", "PLATFORM", "CALLS", "IN", "OUT", "TOTAL"];
        self.print_table_header(&headers);
        for r in rows {
            let plat = self.platform_label(&r.platform, &r.platform);
            self.print_table_row(&[
                &truncate_vis(&r.model, 28),
                &plat,
                &r.call_count.to_string(),
                &format_tokens(r.input_tokens),
                &format_tokens(r.output_tokens),
                &format_tokens(r.total_tokens),
            ]);
        }
        self.rule();
    }

    pub fn success(&self, msg: &str) -> String {
        self.green(msg)
    }

    fn print_table_header(&self, headers: &[&str]) {
        let b = self.boxer();
        let line: String = headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let w = col_width(i, headers.len());
                pad_vis(h, w)
            })
            .collect::<Vec<_>>()
            .join("");
        self.println(b.line(&format!(" {}", self.bold(&line))));
        self.println(b.line(&format!(" {}", self.dim(&"─".repeat(visible_len(&line).min(72))))));
    }

    fn print_table_row(&self, cells: &[&str]) {
        let b = self.boxer();
        let line: String = cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let w = col_width(i, cells.len());
                pad_vis(c, w)
            })
            .collect::<Vec<_>>()
            .join("");
        self.println(b.line(&format!(" {line}")));
    }

    pub fn print_list_platforms(
        &self,
        rows: &[(&str, &str, &str, &str)],
    ) {
        let b = self.boxer();
        self.blank();
        self.println(self.cyan(&b.top(Some("Registered platforms"))));
        for (id, kind, status, name) in rows {
            let label = self.platform_label(id, name);
            let st = match *status {
                "implemented" => self.green("ok"),
                "research" | "stub" => self.yellow("~"),
                _ => self.dim("-"),
            };
            self.println(b.line(&format!(" {label}  {st}  {kind}  {status}")));
        }
        self.println(self.dim(&b.bottom()));
    }

    pub fn print_doctor(
        &self,
        db_path: &str,
        exists: bool,
        event_count: i64,
        adapters: &[DoctorLine],
    ) {
        let b = self.boxer();
        self.blank();
        self.println(self.cyan(&b.top(Some("Doctor"))));
        self.println(b.line(&format!(" Database  {db_path}")));
        self.println(b.line(&format!(
            " Status    {}",
            if exists { self.green("ok") } else { self.red("missing") }
        )));
        self.println(b.line(&format!(
            " Events    {}",
            self.bold(&event_count.to_string())
        )));
        self.println(b.section("Paths"));
        for a in adapters {
            let label = self.platform_label(&a.id, &a.id);
            let badge = if a.found > 0 {
                self.green(&format!("{}/{}", a.found, a.total))
            } else {
                self.dim(&format!("0/{}", a.total))
            };
            self.println(b.line(&format!(" {label}  {badge}")));
            for p in &a.paths {
                self.println(b.line(&format!("   {}", self.dim(p))));
            }
        }
        self.println(self.dim(&b.bottom()));
    }

    pub fn print_probe(&self, platform: &str, display_name: &str, hits: &[ProbeLine]) {
        let b = self.boxer();
        let title = format!("{}  Probe", self.platform_label(platform, display_name));
        self.blank();
        self.println(self.cyan(&b.top(Some(&title))));
        for h in hits {
            let mark = if h.exists { self.green("ok") } else { self.dim("missing") };
            self.println(b.line(&format!(" {mark}  {}", h.path)));
            if let Some(size) = &h.size {
                self.println(b.line(&format!("      {}", self.dim(size))));
            }
            if let Some(note) = &h.note {
                self.println(b.line(&format!("      {}", self.dim(note))));
            }
        }
        self.println(self.dim(&b.bottom()));
    }

    pub fn empty_hint(&self, since_label: &str) {
        let b = self.boxer();
        self.blank();
        self.println(self.cyan(&b.top(Some("No data"))));
        self.println(b.line(&format!(
            " {}",
            self.yellow(&format!("Window '{since_label}' has no events."))
        )));
        self.println(b.line(&format!(" {}", self.dim("Run: tokens scan --full"))));
        self.println(self.dim(&b.bottom()));
    }
}

pub struct DoctorLine {
    pub id: String,
    pub found: usize,
    pub total: usize,
    pub paths: Vec<String>,
}

pub struct ProbeLine {
    pub exists: bool,
    pub path: String,
    pub size: Option<String>,
    pub note: Option<String>,
}

fn col_width(idx: usize, cols: usize) -> usize {
    match (idx, cols) {
        (0, _) => 18,
        (1, n) if n > 6 => 6,
        (2, n) if n > 7 => 10,
        (_, n) if n >= 8 => 10,
        (_, n) if n >= 6 => 8,
        _ => 12,
    }
}
