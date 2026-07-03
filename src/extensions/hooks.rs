//! `ztmux hooks` — the command hooks configured on each session.
//!
//! tmux can run a command automatically when something happens — a window is
//! selected, a pane dies, a client attaches — by binding it to a *hook*. Hooks
//! are invisible until they fire and easy to forget you set. `hooks` lists them:
//! for every session it reads the session-scope hooks (the ones you set, not the
//! built-in global defaults) and reports the event each is bound to and the
//! command it runs. It is the "what automation is wired into this server" view —
//! the config companion to the runtime extensions. Sessions with no custom hook
//! contribute nothing. With `-o json` / `--json` it emits the same rows as a
//! machine-readable array, sorted by session then event.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// One output row: a session, a hook event, and the command bound to it.
struct Row {
    session: String,
    event: String,
    command: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let sessions = query_lines(socket, &["list-sessions", "-F", "#{session_name}"]);
    let mut pairs: Vec<(String, String)> = Vec::new();
    for session in &sessions {
        let session = session.trim();
        if session.is_empty() {
            continue;
        }
        for line in query_lines(socket, &["show-hooks", "-t", session]) {
            pairs.push((session.to_string(), line));
        }
    }
    let rows = build_rows(pairs);
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

/// Parse one `show-hooks` line into `(event, command)`. A line looks like
/// `after-select-window[0] display-message hi`: the first whitespace-separated
/// token is the event (with an optional `[index]` for array hooks, which is
/// stripped), and the rest is the command.
fn parse_hook_line(line: &str) -> Option<(String, String)> {
    let (head, rest) = line.trim().split_once(char::is_whitespace)?;
    // Drop a trailing `[n]` array index from the event name.
    let event = head.split('[').next().unwrap_or(head);
    let command = rest.trim();
    if event.is_empty() || command.is_empty() {
        return None;
    }
    Some((event.to_string(), command.to_string()))
}

/// Build one row per parseable `(session, hook-line)` pair, sorted by session
/// then event.
fn build_rows(pairs: Vec<(String, String)>) -> Vec<Row> {
    let mut rows: Vec<Row> = pairs
        .into_iter()
        .filter_map(|(session, line)| {
            let (event, command) = parse_hook_line(&line)?;
            Some(Row {
                session,
                event,
                command,
            })
        })
        .collect();
    rows.sort_by(|a, b| a.session.cmp(&b.session).then(a.event.cmp(&b.event)));
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
            &format!("{:<12} {:<24} {}", "SESSION", "EVENT", "COMMAND"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<12} {:<24} {}\n",
            r.session, r.event, r.command
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "session": r.session,
                "event": r.event,
                "command": r.command,
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
    fn parses_a_hook_line_with_array_index() {
        let (e, c) = parse_hook_line("after-select-window[0] display-message hi").unwrap();
        assert_eq!(e, "after-select-window");
        assert_eq!(c, "display-message hi");
    }

    #[test]
    fn parses_a_hook_line_without_index() {
        let (e, c) = parse_hook_line("pane-died respawn-pane").unwrap();
        assert_eq!(e, "pane-died");
        assert_eq!(c, "respawn-pane");
    }

    #[test]
    fn rejects_malformed_lines() {
        assert!(parse_hook_line("").is_none());
        assert!(parse_hook_line("just-an-event").is_none()); // no command
    }

    #[test]
    fn build_rows_sorts_by_session_then_event() {
        let pairs = vec![
            ("work".to_string(), "pane-died[0] respawn-pane".to_string()),
            (
                "work".to_string(),
                "after-select-window[0] display x".to_string(),
            ),
            (
                "admin".to_string(),
                "client-attached[0] display y".to_string(),
            ),
        ];
        let rows = build_rows(pairs);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].session, "admin");
        // Within "work", after-select-window sorts before pane-died.
        assert_eq!(rows[1].event, "after-select-window");
        assert_eq!(rows[2].event, "pane-died");
    }

    #[test]
    fn json_carries_session_event_command() {
        let pairs = vec![("a".to_string(), "pane-died respawn-pane".to_string())];
        let rows = build_rows(pairs);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["session"], "a");
        assert_eq!(v[0]["event"], "pane-died");
        assert_eq!(v[0]["command"], "respawn-pane");
    }
}
