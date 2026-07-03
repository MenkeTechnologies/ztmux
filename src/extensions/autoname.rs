//! `ztmux autoname` — windows whose name is pinned (automatic-rename off).
//!
//! By default a window renames itself to follow its active command
//! (`automatic-rename` on). Turning it off pins the name: it stays whatever you
//! set no matter what runs there. `autoname` reports the windows where
//! automatic-rename is off — the ones with a name you have deliberately fixed.
//! Where [`super::named`] infers intent by comparing a window's name to its
//! command, `autoname` reads the option directly, so it also catches a pinned
//! name that happens to match the command right now. It is the "which window
//! names are frozen" view. Windows with the default (auto-rename on) are omitted.
//! With `-o json` / `--json` it emits the same rows as a machine-readable array;
//! a server with none pinned prints just the header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-window format the option is read through.
const FORMAT: &str =
    "#{session_name}\u{1f}#{window_index}\u{1f}#{window_name}\u{1f}#{automatic-rename}";

/// One output row: a window with a pinned (non-auto) name.
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

/// Parse one formatted line into `(location, name, auto_rename)`.
/// `automatic-rename` is `1` (on, the default) or `0` (off, pinned).
fn parse_line(line: &str) -> Option<(String, String, bool)> {
    let mut it = line.split('\u{1f}');
    let session = it.next()?;
    let index = it.next()?;
    let name = it.next()?;
    let auto = it.next()? == "1";
    Some((format!("{session}:{index}"), name.to_string(), auto))
}

/// One row per window whose name is pinned (automatic-rename off), ordered by
/// location.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, _, auto)| !*auto)
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
        let (loc, name, auto) = parse_line("a\u{1f}2\u{1f}deploy\u{1f}0").unwrap();
        assert_eq!(loc, "a:2");
        assert_eq!(name, "deploy");
        assert!(!auto);
    }

    #[test]
    fn only_pinned_windows_are_kept() {
        let lines = vec![
            "a\u{1f}0\u{1f}auto\u{1f}1".to_string(), // auto-rename on -> omitted
            "a\u{1f}1\u{1f}pinned\u{1f}0".to_string(), // off -> kept
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].location, "a:1");
        assert_eq!(rows[0].name, "pinned");
    }

    #[test]
    fn none_pinned_renders_header_only() {
        let lines = vec!["a\u{1f}0\u{1f}auto\u{1f}1".to_string()];
        let rows = build_rows(&lines);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("WINDOW") && s.contains("NAME"));
    }

    #[test]
    fn json_carries_window_and_name() {
        let lines = vec!["a\u{1f}1\u{1f}pinned\u{1f}0".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["window"], "a:1");
        assert_eq!(v[0]["name"], "pinned");
    }
}
