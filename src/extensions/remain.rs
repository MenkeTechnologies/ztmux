//! `ztmux remain` — windows that keep a pane's shell around after it exits.
//!
//! With `remain-on-exit` set, a pane whose process exits is not closed but held
//! open as a dead pane (`on` always, `failed` only on a non-zero exit) — so its
//! last output stays on screen and it can be respawned. `remain` reports the
//! windows where that option is armed. Where [`super::dead`] lists the panes that
//! have *already* died and [`super::respawn`] revives them, `remain` shows which
//! windows are *configured* to hold their dead panes in the first place. It is
//! the "which windows won't close on exit" view. Windows with the default
//! (`off`) are omitted. With `-o json` / `--json` it emits the same rows as a
//! machine-readable array; a server with none armed prints just the header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-window format the option is read through.
const FORMAT: &str =
    "#{session_name}\u{1f}#{window_index}\u{1f}#{window_name}\u{1f}#{remain-on-exit}";

/// One output row: a window with remain-on-exit armed.
struct Row {
    location: String, // session:index
    name: String,
    mode: String, // the option value, e.g. "on" or "failed"
}

pub(crate) fn run(socket: &str) -> i32 {
    let lines = query_lines(socket, &["list-windows", "-a", "-F", FORMAT]);
    let rows = build_rows(&lines);
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

/// Parse one formatted line into `(location, name, mode)`. `remain-on-exit`
/// reports a string: `off` (default), `on`, or `failed`.
fn parse_line(line: &str) -> Option<(String, String, String)> {
    let mut it = line.split('\u{1f}');
    let session = it.next()?;
    let index = it.next()?;
    let name = it.next()?;
    let mode = it.next()?;
    Some((
        format!("{session}:{index}"),
        name.to_string(),
        mode.to_string(),
    ))
}

/// One row per window whose `remain-on-exit` is anything but `off`, ordered by
/// location.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, _, mode)| mode != "off")
        .map(|(location, name, mode)| Row {
            location,
            name,
            mode,
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
        paint(&format!("{:<12} {:<16} {}", "WINDOW", "NAME", "MODE"), "1")
    ));
    for r in rows {
        out.push_str(&format!("{:<12} {:<16} {}\n", r.location, r.name, r.mode));
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
                "mode": r.mode,
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
    fn parses_a_formatted_line() {
        let (loc, name, mode) = parse_line("a\u{1f}3\u{1f}logs\u{1f}on").unwrap();
        assert_eq!(loc, "a:3");
        assert_eq!(name, "logs");
        assert_eq!(mode, "on");
    }

    #[test]
    fn off_windows_are_omitted_on_and_failed_kept() {
        let lines = vec![
            "a\u{1f}0\u{1f}normal\u{1f}off".to_string(),
            "a\u{1f}1\u{1f}kept\u{1f}on".to_string(),
            "a\u{1f}2\u{1f}onfail\u{1f}failed".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].location, "a:1");
        assert_eq!(rows[0].mode, "on");
        assert_eq!(rows[1].mode, "failed");
    }

    #[test]
    fn none_armed_renders_header_only() {
        let lines = vec!["a\u{1f}0\u{1f}normal\u{1f}off".to_string()];
        let rows = build_rows(&lines);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("WINDOW") && s.contains("MODE"));
    }

    #[test]
    fn json_carries_mode() {
        let lines = vec!["a\u{1f}1\u{1f}kept\u{1f}on".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["window"], "a:1");
        assert_eq!(v[0]["mode"], "on");
    }
}
