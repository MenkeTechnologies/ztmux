//! `ztmux limit` — each session's scrollback capacity, largest first.
//!
//! Every session has a `history-limit`: the maximum number of lines a pane keeps
//! in its scrollback. Where [`super::history`] ranks panes by the scrollback they
//! are *using* right now, `limit` reports the configured *cap* per session and
//! ranks by it, biggest first — the capacity setting behind the usage. It
//! surfaces the sessions with an unusually large (memory-hungry) or small
//! (truncating) buffer configured. It reads the option for every session. With
//! `-o json` / `--json` it emits the same rows as a machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-session format the option is read through.
const FORMAT: &str = "#{session_name}\u{1f}#{history-limit}";

/// One output row: a session and its configured scrollback cap.
struct Row {
    session: String,
    limit: i64,
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

/// Parse one formatted line into `(session, history_limit)`.
fn parse_line(line: &str) -> Option<(String, i64)> {
    let mut it = line.split('\u{1f}');
    let session = it.next()?;
    let limit: i64 = it.next()?.parse().ok()?;
    Some((session.to_string(), limit))
}

/// One row per session, largest scrollback cap first; ties break by session name.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .map(|(session, limit)| Row { session, limit })
        .collect();
    rows.sort_by(|a, b| b.limit.cmp(&a.limit).then(a.session.cmp(&b.session)));
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
        paint(&format!("{:>9} {}", "LIMIT", "SESSION"), "1")
    ));
    for r in rows {
        out.push_str(&format!("{:>9} {}\n", r.limit, r.session));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "session": r.session,
                "limit": r.limit,
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
        assert_eq!(
            parse_line("work\u{1f}50000"),
            Some(("work".to_string(), 50000))
        );
        assert_eq!(parse_line("bad"), None);
    }

    #[test]
    fn largest_limit_sorts_first() {
        let lines = vec![
            "small\u{1f}2000".to_string(),
            "big\u{1f}100000".to_string(),
            "mid\u{1f}30000".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows[0].session, "big");
        assert_eq!(rows[0].limit, 100000);
        assert_eq!(rows[2].session, "small");
    }

    #[test]
    fn equal_limits_break_ties_by_name() {
        let lines = vec!["bbb\u{1f}30000".to_string(), "aaa\u{1f}30000".to_string()];
        let rows = build_rows(&lines);
        assert_eq!(rows[0].session, "aaa");
        assert_eq!(rows[1].session, "bbb");
    }

    #[test]
    fn json_carries_session_and_limit() {
        let lines = vec!["work\u{1f}50000".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["session"], "work");
        assert_eq!(v[0]["limit"], 50000);
    }
}
