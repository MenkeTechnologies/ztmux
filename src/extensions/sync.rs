//! `ztmux sync` — windows with `synchronize-panes` turned on.
//!
//! When a window has `synchronize-panes` on, every keystroke goes to *all* its
//! panes at once — powerful for driving a fleet, dangerous when you have
//! forgotten it is on and start typing. Nothing in the normal views shows the
//! state, so it is easy to lose track across a wall of windows. `sync` surfaces
//! it: it reads the `pane_synchronized` flag for every window and reports the
//! ones where it is set. It is the safety check before you type. Windows without
//! synchronize-panes are omitted. With `-o json` / `--json` it emits the same
//! rows as a machine-readable array; a server with none synchronized prints just
//! the header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-window format the flag is read through.
const FORMAT: &str =
    "#{session_name}\u{1f}#{window_index}\u{1f}#{window_name}\u{1f}#{?pane_synchronized,1,0}";

/// One output row: a window with synchronize-panes enabled.
struct Row {
    location: String, // session:index
    name: String,
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

/// Parse one formatted line into `(location, name, synchronized)`.
fn parse_line(line: &str) -> Option<(String, String, bool)> {
    let mut it = line.split('\u{1f}');
    let session = it.next()?;
    let index = it.next()?;
    let name = it.next()?;
    let sync = it.next()? == "1";
    Some((format!("{session}:{index}"), name.to_string(), sync))
}

/// One row per synchronized window, ordered by location.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, _, sync)| *sync)
        .map(|(location, name, _)| Row { location, name })
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
        paint(&format!("{:<12} {}", "WINDOW", "NAME"), "1")
    ));
    for r in rows {
        out.push_str(&format!("{:<12} {}\n", r.location, r.name));
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
        let (loc, name, sync) = parse_line("work\u{1f}2\u{1f}build\u{1f}1").unwrap();
        assert_eq!(loc, "work:2");
        assert_eq!(name, "build");
        assert!(sync);
    }

    #[test]
    fn only_synchronized_windows_are_kept() {
        let lines = vec![
            "a\u{1f}0\u{1f}one\u{1f}1".to_string(),
            "a\u{1f}1\u{1f}two\u{1f}0".to_string(),
            "b\u{1f}0\u{1f}three\u{1f}1".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].location, "a:0");
        assert_eq!(rows[1].location, "b:0");
    }

    #[test]
    fn rows_sorted_by_location() {
        let lines = vec![
            "z\u{1f}9\u{1f}late\u{1f}1".to_string(),
            "a\u{1f}0\u{1f}early\u{1f}1".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows[0].location, "a:0");
        assert_eq!(rows[1].location, "z:9");
    }

    #[test]
    fn none_synchronized_renders_header_only() {
        let lines = vec!["a\u{1f}0\u{1f}one\u{1f}0".to_string()];
        let rows = build_rows(&lines);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("WINDOW") && s.contains("NAME"));
    }

    #[test]
    fn json_carries_window_and_name() {
        let lines = vec!["a\u{1f}0\u{1f}one\u{1f}1".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["window"], "a:0");
        assert_eq!(v[0]["name"], "one");
    }
}
