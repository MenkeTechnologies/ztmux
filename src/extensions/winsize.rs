//! `ztmux winsize` — windows whose sizing mode differs from the default.
//!
//! `window-size` decides how a window is sized when several clients of different
//! sizes are attached: `latest` (follow the most-recent client), `largest`,
//! `smallest`, or `manual` (a fixed size you set with `resize-window`). Most
//! windows inherit the global default; `winsize` reports the ones that do not —
//! the windows you have deliberately given a different sizing rule. It reads the
//! global default once and lists every window whose own `window-size` differs
//! from it. Where [`super::constrain`] shows which *client* caps a session,
//! `winsize` shows which *windows* opt out of the default sizing. Windows on the
//! default are omitted. With `-o json` / `--json` it emits the same rows as a
//! machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-window format the option is read through.
const FORMAT: &str = "#{session_name}\u{1f}#{window_index}\u{1f}#{window_name}\u{1f}#{window-size}";

/// One output row: a window with a non-default sizing mode.
struct Row {
    location: String, // session:index
    name: String,
    size: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let global = global_default(socket);
    let lines = query_lines(socket, &["list-windows", "-a", "-F", FORMAT]);
    let rows = build_rows(&lines, &global);
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

/// Read the global `window-size` default via `show-options`, whose line looks
/// like `window-size latest`. Falls back to `latest` (tmux's built-in default)
/// when it cannot be read.
fn global_default(socket: &str) -> String {
    query_lines(socket, &["show-options", "-g", "-w", "window-size"])
        .first()
        .and_then(|l| parse_option_value(l))
        .unwrap_or_else(|| "latest".to_string())
}

/// Parse the value from a `show-options` line (`window-size latest` → `latest`).
fn parse_option_value(line: &str) -> Option<String> {
    line.split_whitespace().nth(1).map(str::to_string)
}

/// Parse one formatted line into `(location, name, size)`.
fn parse_line(line: &str) -> Option<(String, String, String)> {
    let mut it = line.split('\u{1f}');
    let session = it.next()?;
    let index = it.next()?;
    let name = it.next()?;
    let size = it.next()?;
    Some((
        format!("{session}:{index}"),
        name.to_string(),
        size.to_string(),
    ))
}

/// One row per window whose `window-size` differs from the global default,
/// ordered by location.
fn build_rows(lines: &[String], global: &str) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, _, size)| size != global)
        .map(|(location, name, size)| Row {
            location,
            name,
            size,
        })
        .collect();
    rows.sort_by(|a, b| a.location.cmp(&b.location));
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
        paint(&format!("{:<12} {:<16} {}", "WINDOW", "NAME", "SIZE"), "1")
    ));
    for r in rows {
        out.push_str(&format!("{:<12} {:<16} {}\n", r.location, r.name, r.size));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "window": r.location,
                "name": r.name,
                "size": r.size,
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

    #[test]
    fn parses_option_value() {
        assert_eq!(
            parse_option_value("window-size latest"),
            Some("latest".to_string())
        );
        assert_eq!(parse_option_value("window-size"), None);
    }

    #[test]
    fn parses_a_formatted_line() {
        let (loc, name, size) = parse_line("a\u{1f}1\u{1f}build\u{1f}manual").unwrap();
        assert_eq!(loc, "a:1");
        assert_eq!(name, "build");
        assert_eq!(size, "manual");
    }

    #[test]
    fn only_windows_differing_from_global_are_kept() {
        let lines = vec![
            "a\u{1f}0\u{1f}def\u{1f}latest".to_string(), // matches global -> omitted
            "a\u{1f}1\u{1f}man\u{1f}manual".to_string(), // differs -> kept
            "a\u{1f}2\u{1f}big\u{1f}largest".to_string(), // differs -> kept
        ];
        let rows = build_rows(&lines, "latest");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].location, "a:1");
        assert_eq!(rows[0].size, "manual");
        assert_eq!(rows[1].size, "largest");
    }

    #[test]
    fn all_default_renders_header_only() {
        let lines = vec!["a\u{1f}0\u{1f}def\u{1f}latest".to_string()];
        let rows = build_rows(&lines, "latest");
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("WINDOW") && s.contains("SIZE"));
    }

    #[test]
    fn json_carries_window_and_size() {
        let lines = vec!["a\u{1f}1\u{1f}man\u{1f}manual".to_string()];
        let rows = build_rows(&lines, "latest");
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["window"], "a:1");
        assert_eq!(v[0]["size"], "manual");
    }
}
