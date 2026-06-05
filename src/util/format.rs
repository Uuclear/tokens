pub fn format_tokens(n: i64) -> String {
    let n = n.max(0) as f64;
    if n >= 1_000_000_000.0 {
        format!("{:.2}b", n / 1_000_000_000.0)
    } else if n >= 1_000_000.0 {
        format!("{:.2}m", n / 1_000_000.0)
    } else if n >= 1_000.0 {
        format!("{:.2}k", n / 1_000.0)
    } else {
        format!("{n:.0}")
    }
}

pub fn format_duration_secs(secs: i64) -> String {
    if secs < 60 {
        return format!("{secs}s");
    }
    if secs < 3600 {
        return format!("{}m {}s", secs / 60, secs % 60);
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h}h {m}m {s}s")
}
