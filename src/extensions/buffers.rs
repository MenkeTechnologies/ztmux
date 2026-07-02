//! `ztmux buffers` — the server's paste buffers, largest first.
//!
//! tmux keeps a stack of paste buffers — everything copied in copy-mode or
//! loaded with `set-buffer`/`load-buffer` — but the normal views never show
//! them. `buffers` does: it lists every buffer with its size and a one-line
//! preview of its contents, ranked biggest first, so you can see what you have
//! on the clipboard stack before pasting. It reads the same
//! `list-buffers -o json` machine output the port already emits. With `-o json`
//! / `--json` it emits the same rows (full sample retained) as a machine-readable
//! array; a server with no buffers prints just the header.

use std::io::IsTerminal;

use serde::Deserialize;

use super::tmux_query::run_json;

/// A paste buffer as reported by `list-buffers -o json`.
#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct Buffer {
    name: String,
    size: i64,
    sample: String,
}

/// One output row: a paste buffer with a sanitised preview.
struct Row {
    name: String,
    size: i64,
    preview: String,
    sample: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let buffers = match run_json::<Buffer>(socket, &["list-buffers", "-o", "json"]) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ztmux buffers: {e}");
            return 1;
        }
    };
    let rows = build_rows(buffers);
    let json = std::env::args().any(|a| a == "--json")
        || std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] == "-o" && w[1] == "json");
    if json {
        print!("{}", render_json(&rows));
    } else {
        print!("{}", render_text(&rows, std::io::stdout().is_terminal()));
    }
    0
}

/// Collapse a buffer sample to a single printable line: control characters
/// (newlines, tabs, …) become spaces, and the result is truncated to 60
/// characters with an ellipsis so a big buffer stays one row wide.
fn preview(sample: &str) -> String {
    let cleaned: String = sample
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    const MAX: usize = 60;
    if cleaned.chars().count() > MAX {
        let head: String = cleaned.chars().take(MAX - 1).collect();
        format!("{head}…")
    } else {
        cleaned
    }
}

/// Human-readable byte size (`B`/`K`/`M`).
fn fmt_bytes(n: i64) -> String {
    if n >= 1_048_576 {
        format!("{:.1}M", n as f64 / 1_048_576.0)
    } else if n >= 1024 {
        format!("{:.1}K", n as f64 / 1024.0)
    } else {
        format!("{n}B")
    }
}

/// One row per buffer, largest first; ties break by name for a stable order.
fn build_rows(buffers: Vec<Buffer>) -> Vec<Row> {
    let mut rows: Vec<Row> = buffers
        .into_iter()
        .map(|b| Row {
            preview: preview(&b.sample),
            name: b.name,
            size: b.size,
            sample: b.sample,
        })
        .collect();
    rows.sort_by(|a, b| b.size.cmp(&a.size).then(a.name.cmp(&b.name)));
    rows
}

fn render_text(rows: &[Row], color: bool) -> String {
    let paint = |s: &str, code: &str| -> String {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let mut out = String::new();
    out.push_str(&format!(
        "{}\n",
        paint(
            &format!("{:<16} {:>8} {}", "BUFFER", "SIZE", "PREVIEW"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<16} {:>8} {}\n",
            r.name,
            fmt_bytes(r.size),
            r.preview,
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "name": r.name,
                "size": r.size,
                "sample": r.sample,
            })
        })
        .collect();
    format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::Value::Array(arr)).unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(name: &str, size: i64, sample: &str) -> Buffer {
        Buffer {
            name: name.into(),
            size,
            sample: sample.into(),
        }
    }

    #[test]
    fn largest_buffer_sorts_first() {
        let rows = build_rows(vec![
            buffer("buffer0", 11, "hello"),
            buffer("buffer1", 26, "second buffer content"),
        ]);
        assert_eq!(rows[0].name, "buffer1");
        assert_eq!(rows[0].size, 26);
        assert_eq!(rows[1].name, "buffer0");
    }

    #[test]
    fn preview_collapses_control_chars() {
        assert_eq!(preview("line one\nline two\ttab"), "line one line two tab");
    }

    #[test]
    fn preview_truncates_long_samples_with_ellipsis() {
        let long = "x".repeat(100);
        let p = preview(&long);
        assert_eq!(p.chars().count(), 60);
        assert!(p.ends_with('…'));
    }

    #[test]
    fn fmt_bytes_scales_units() {
        assert_eq!(fmt_bytes(512), "512B");
        assert_eq!(fmt_bytes(2048), "2.0K");
        assert_eq!(fmt_bytes(3_145_728), "3.0M");
    }

    #[test]
    fn no_buffers_renders_header_only() {
        let rows = build_rows(vec![]);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("BUFFER") && s.contains("PREVIEW"));
    }

    #[test]
    fn json_retains_full_sample() {
        let rows = build_rows(vec![buffer("buffer0", 5, "a\nb\nc")]);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["name"], "buffer0");
        assert_eq!(v[0]["size"], 5);
        assert_eq!(v[0]["sample"], "a\nb\nc"); // newlines preserved in JSON
    }
}
