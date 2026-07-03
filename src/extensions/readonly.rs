//! `ztmux readonly` — clients attached in read-only mode.
//!
//! A client attached with `attach -r` (or `switch-client -r`) can see a session
//! but cannot type into it — a safe way to let someone observe, or to watch a
//! session you do not want to disturb. Nothing in the normal views distinguishes
//! a read-only client from a live one. `readonly` reads the `client_readonly`
//! flag for every attached client and reports the view-only ones. Where
//! [`super::who`] lists every client, `readonly` isolates the observers. Clients
//! that can type are omitted. With `-o json` / `--json` it emits the same rows as
//! a machine-readable array; a server with no read-only clients prints just the
//! header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-client format the flag is read through.
const FORMAT: &str = "#{client_name}\u{1f}#{client_session}\u{1f}#{client_readonly}";

/// One output row: a read-only client and the session it is viewing.
struct Row {
    client: String,
    session: String,
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

/// Parse one formatted line into `(client, session, readonly)`.
fn parse_line(line: &str) -> Option<(String, String, bool)> {
    let mut it = line.split('\u{1f}');
    let client = it.next()?;
    let session = it.next()?;
    let readonly = it.next()? == "1";
    Some((client.to_string(), session.to_string(), readonly))
}

/// One row per read-only client, ordered by client name.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, _, ro)| *ro)
        .map(|(client, session, _)| Row { client, session })
        .collect();
    rows.sort_by(|a, b| a.client.cmp(&b.client));
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
        paint(&format!("{:<18} {}", "CLIENT", "SESSION"), "1")
    ));
    for r in rows {
        out.push_str(&format!("{:<18} {}\n", r.client, r.session));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "client": r.client,
                "session": r.session,
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
        let (c, s, ro) = parse_line("/dev/ttys1\u{1f}work\u{1f}1").unwrap();
        assert_eq!(c, "/dev/ttys1");
        assert_eq!(s, "work");
        assert!(ro);
    }

    #[test]
    fn only_read_only_clients_are_kept() {
        let lines = vec![
            "/dev/ttys1\u{1f}a\u{1f}0".to_string(), // can type
            "/dev/ttys2\u{1f}b\u{1f}1".to_string(), // read-only
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].client, "/dev/ttys2");
        assert_eq!(rows[0].session, "b");
    }

    #[test]
    fn none_readonly_renders_header_only() {
        let lines = vec!["/dev/ttys1\u{1f}a\u{1f}0".to_string()];
        let rows = build_rows(&lines);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("CLIENT") && s.contains("SESSION"));
    }

    #[test]
    fn json_carries_client_and_session() {
        let lines = vec!["/dev/ttys2\u{1f}b\u{1f}1".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["client"], "/dev/ttys2");
        assert_eq!(v[0]["session"], "b");
    }
}
