//! `ztmux borders` — windows that draw a status line on their pane borders.
//!
//! `pane-border-status` puts a one-line label on each pane's border — `top` or
//! `bottom` — usually showing the pane's title or command, so a split window
//! self-documents which pane is which. It is off by default. `borders` reports
//! the windows where it is enabled and on which edge. It is a small awareness
//! view for a per-window display setting: which windows are showing their pane
//! labels. Windows with the default (`off`) are omitted. With `-o json` /
//! `--json` it emits the same rows as a machine-readable array; a server with
//! none enabled prints just the header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-window format the option is read through.
const FORMAT: &str =
    "#{session_name}\u{1f}#{window_index}\u{1f}#{window_name}\u{1f}#{pane-border-status}";

/// One output row: a window drawing a pane-border status line.
struct Row {
    location: String, // session:index
    name: String,
    position: String, // "top" or "bottom"
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

/// Parse one formatted line into `(location, name, position)`.
/// `pane-border-status` reports `off`, `top`, or `bottom`.
fn parse_line(line: &str) -> Option<(String, String, String)> {
    let mut it = line.split('\u{1f}');
    let session = it.next()?;
    let index = it.next()?;
    let name = it.next()?;
    let position = it.next()?;
    Some((
        format!("{session}:{index}"),
        name.to_string(),
        position.to_string(),
    ))
}

/// One row per window whose pane-border status is not `off`, ordered by location.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, _, position)| position != "off")
        .map(|(location, name, position)| Row {
            location,
            name,
            position,
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
        paint(
            &format!("{:<12} {:<16} {}", "WINDOW", "NAME", "BORDER"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<12} {:<16} {}\n",
            r.location, r.name, r.position
        ));
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
                "position": r.position,
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
        let (loc, name, pos) = parse_line("a\u{1f}0\u{1f}dev\u{1f}top").unwrap();
        assert_eq!(loc, "a:0");
        assert_eq!(name, "dev");
        assert_eq!(pos, "top");
    }

    #[test]
    fn only_enabled_windows_are_kept() {
        let lines = vec![
            "a\u{1f}0\u{1f}off-win\u{1f}off".to_string(),
            "a\u{1f}1\u{1f}top-win\u{1f}top".to_string(),
            "a\u{1f}2\u{1f}bot-win\u{1f}bottom".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].position, "top");
        assert_eq!(rows[1].position, "bottom");
    }

    #[test]
    fn none_enabled_renders_header_only() {
        let lines = vec!["a\u{1f}0\u{1f}off-win\u{1f}off".to_string()];
        let rows = build_rows(&lines);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("WINDOW") && s.contains("BORDER"));
    }

    #[test]
    fn json_carries_position() {
        let lines = vec!["a\u{1f}1\u{1f}top-win\u{1f}top".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["window"], "a:1");
        assert_eq!(v[0]["position"], "top");
    }
}
