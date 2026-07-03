//! `ztmux viewers` — how many clients are attached to each session.
//!
//! A session can have several clients on it at once — a paired session, a remote
//! observer, the same session open from two machines. `viewers` counts the
//! clients attached to each session and ranks the sessions by that count,
//! most-watched first. Where [`super::who`] lists the clients individually and
//! [`super::fanout`] ranks sessions by pane count, `viewers` ranks them by *eyes
//! on them* — the "which session is being watched, and by how many" view.
//! Sessions with no client attached do not appear (only attached clients are
//! counted). With `-o json` / `--json` it emits the same rows as a
//! machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The per-client format: just the session each client is attached to.
const FORMAT: &str = "#{client_session}";

/// One output row: a session and how many clients are attached to it.
struct Row {
    session: String,
    clients: usize,
}

pub(crate) fn run(socket: &str) -> i32 {
    let lines = query_lines(socket, &["list-clients", "-F", FORMAT]);
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

/// Count attached clients per session, most-watched first. A
/// [`std::collections::BTreeMap`] gives a deterministic session order; a stable
/// count-descending sort then puts the most-watched on top. Blank session names
/// (a client not yet attached to a session) are skipped.
fn build_rows(lines: &[String]) -> Vec<Row> {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for line in lines {
        let session = line.trim();
        if session.is_empty() {
            continue;
        }
        *counts.entry(session.to_string()).or_default() += 1;
    }
    let mut rows: Vec<Row> = counts
        .into_iter()
        .map(|(session, clients)| Row { session, clients })
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.clients));
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
        paint(&format!("{:>7} {}", "CLIENTS", "SESSION"), "1")
    ));
    for r in rows {
        out.push_str(&format!("{:>7} {}\n", r.clients, r.session));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "session": r.session,
                "clients": r.clients,
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
    fn most_watched_session_sorts_first() {
        let lines = vec!["work".to_string(), "work".to_string(), "admin".to_string()];
        let rows = build_rows(&lines);
        assert_eq!(rows[0].session, "work");
        assert_eq!(rows[0].clients, 2);
        assert_eq!(rows[1].session, "admin");
        assert_eq!(rows[1].clients, 1);
    }

    #[test]
    fn blank_sessions_are_skipped() {
        let lines = vec!["".to_string(), "a".to_string()];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session, "a");
    }

    #[test]
    fn no_clients_renders_header_only() {
        let rows = build_rows(&[]);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("CLIENTS") && s.contains("SESSION"));
    }

    #[test]
    fn json_carries_session_and_count() {
        let lines = vec!["a".to_string(), "a".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["session"], "a");
        assert_eq!(v[0]["clients"], 2);
    }
}
