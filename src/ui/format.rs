//! Number formatting, as in the WebView UI (format.ts).

use super::i18n::decimal_comma;

pub const KB: f64 = 1024.0;
pub const MB: f64 = KB * 1024.0;
pub const GB: f64 = MB * 1024.0;

fn fixed(n: f64, digits: usize) -> String {
    let s = format!("{n:.digits$}");
    if decimal_comma() { s.replace('.', ",") } else { s }
}

pub fn f1(n: f64) -> String {
    fixed(n, 1)
}

pub fn f2(n: f64) -> String {
    fixed(n, 2)
}

pub fn pct(n: f64) -> String {
    format!("{}%", n.round() as i64)
}

pub fn bytes(b: u64) -> String {
    let b = b as f64;
    if b >= GB {
        format!("{} GB", f1(b / GB))
    } else if b >= MB {
        format!("{} MB", (b / MB).round())
    } else {
        format!("{} KB", (b / KB).round())
    }
}

pub fn rate(b: f64) -> String {
    if b >= MB { format!("{} MB/s", f1(b / MB)) } else { format!("{} KB/s", (b / KB).round()) }
}

pub fn uptime(secs: u64) -> String {
    let (d, h, m) = (secs / 86400, secs % 86400 / 3600, secs % 3600 / 60);
    if d > 0 { format!("{d}d {h}h") } else { format!("{h}h {m}m") }
}

/// "AMD Ryzen 9 7950X 16-Core Processor" -> "Ryzen 9 7950X".
pub fn short_cpu_name(name: &str) -> String {
    let mut s = name.strip_prefix("AMD ").or_else(|| name.strip_prefix("Intel(R) Core(TM) ")).unwrap_or(name);
    if let Some(rest) = s.strip_suffix("-Core Processor") {
        if let Some(i) = rest.rfind(' ') {
            if rest[i + 1..].chars().all(|c| c.is_ascii_digit()) {
                s = &s[..i];
            }
        }
    }
    s.strip_suffix(" Processor").unwrap_or(s).to_string()
}
