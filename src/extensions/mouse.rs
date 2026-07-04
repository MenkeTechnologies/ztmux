//! `ztmux mouse` — which sessions have mouse mode enabled.
//!
//! The `mouse` option (off by default) makes tmux capture mouse events to
//! select panes, resize borders by dragging, and scroll into copy mode. It is a
//! session option, so it can be on in one session and off in another, and mouse
//! mode also changes how the *inner* program sees clicks (tmux intercepts them
//! rather than forwarding), which is exactly the surprise when a TUI's own mouse
//! handling "stops working". `mouse` lists every session with an `on`/`off`
//! column so the state is visible at a glance. Both states are reported (an
//! unfiltered view, like [`super::utf8`]), ordered by session name. With
//! `-o json` / `--json` it emits the same rows as a machine-readable array; a
//! server with no sessions prints just the header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-session format the fields are read through.
const FORMAT: &str = "#{session_name}\u{1f}#{mouse}";

/// One output row: a session and its mouse state.
struct Row {
    session: String,
    mouse: bool,
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

/// Interpret the `mouse` option value. `mouse` is a flag option, so tmux
/// formats it as `1`/`0` (not the `on`/`off` string a *choice* option yields);
/// accept both spellings so the reading is robust.
fn is_on(value: &str) -> bool {
    value == "1" || value == "on"
}

/// Parse one formatted line into `(session, mouse)`.
fn parse_line(line: &str) -> Option<(String, bool)> {
    let mut it = line.split('\u{1f}');
    let session = it.next()?;
    let mouse = is_on(it.next()?);
    Some((session.to_string(), mouse))
}

/// One row per session (mouse on or off), ordered by session name.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .map(|(session, mouse)| Row { session, mouse })
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
    let yn = |b: bool| if b { "on" } else { "off" };
    let mut out = String::new();
    out.push_str(&format!(
        "{}\n",
        paint(&format!("{:<16} {}", "SESSION", "MOUSE"), "1")
    ));
    for r in rows {
        out.push_str(&format!("{:<16} {}\n", r.session, yn(r.mouse)));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "session": r.session,
                "mouse": r.mouse,
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
    fn flag_and_choice_spellings_both_count() {
        assert!(is_on("1"));
        assert!(is_on("on"));
        assert!(!is_on("0"));
        assert!(!is_on("off"));
    }

    #[test]
    fn parses_a_formatted_line() {
        let (s, m) = parse_line("work\u{1f}1").unwrap();
        assert_eq!(s, "work");
        assert!(m);
    }

    #[test]
    fn both_states_are_listed_sorted() {
        let lines = vec!["b\u{1f}0".to_string(), "a\u{1f}1".to_string()];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].session, "a");
        assert!(rows[0].mouse);
        assert_eq!(rows[1].session, "b");
        assert!(!rows[1].mouse);
    }

    #[test]
    fn no_sessions_renders_header_only() {
        let rows = build_rows(&[]);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("SESSION") && s.contains("MOUSE"));
    }

    #[test]
    fn json_carries_mouse_flag() {
        let lines = vec!["work\u{1f}1".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["session"], "work");
        assert_eq!(v[0]["mouse"], true);
    }
}
