//! `ztmux status` — sessions with the status line turned off.
//!
//! The status line at the bottom of the screen is on by default, but a session
//! can hide it (`set-option status off`) for a full-height view — and then it is
//! easy to forget which sessions have it off and wonder where the tabs went.
//! `status` reports the sessions where the status line is disabled. It is a
//! small awareness view for a per-session display setting that nothing else
//! surfaces. Sessions showing the status line (the default) are omitted. With
//! `-o json` / `--json` it emits the same rows as a machine-readable array; a
//! server where every session shows its status line prints just the header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-session format the option is read through.
const FORMAT: &str = "#{session_name}\u{1f}#{status}";

/// One output row: a session with its status line hidden.
struct Row {
    session: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let lines = query_lines(socket, &["list-sessions", "-F", FORMAT]);
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

/// Parse one formatted line into `(session, status_on)`. The `status` option
/// reports `on`/`off` (or a numeric line count `2`/`3` for a multi-line status,
/// which is treated as on).
fn parse_line(line: &str) -> Option<(String, bool)> {
    let mut it = line.split('\u{1f}');
    let session = it.next()?;
    let value = it.next()?;
    let on = value != "off" && value != "0";
    Some((session.to_string(), on))
}

/// One row per session whose status line is off, ordered by session name.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, on)| !*on)
        .map(|(session, _)| Row { session })
        .collect();
    rows.sort_by(|a, b| a.session.cmp(&b.session));
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
    out.push_str(&format!("{}\n", paint("SESSION", "1")));
    for r in rows {
        out.push_str(&format!("{}\n", r.session));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| serde_json::json!({ "session": r.session }))
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
    fn off_is_detected_on_and_numeric_are_on() {
        assert_eq!(parse_line("a\u{1f}off"), Some(("a".to_string(), false)));
        assert_eq!(parse_line("a\u{1f}on"), Some(("a".to_string(), true)));
        assert_eq!(parse_line("a\u{1f}2"), Some(("a".to_string(), true)));
        assert_eq!(parse_line("a\u{1f}0"), Some(("a".to_string(), false)));
    }

    #[test]
    fn only_status_off_sessions_are_kept() {
        let lines = vec!["a\u{1f}on".to_string(), "b\u{1f}off".to_string()];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session, "b");
    }

    #[test]
    fn all_on_renders_header_only() {
        let lines = vec!["a\u{1f}on".to_string()];
        let rows = build_rows(&lines);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("SESSION"));
    }

    #[test]
    fn json_carries_session() {
        let lines = vec!["b\u{1f}off".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["session"], "b");
    }
}
