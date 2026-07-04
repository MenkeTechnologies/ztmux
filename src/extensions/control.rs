//! `ztmux control` — clients attached in control mode (`-CC`).
//!
//! A control-mode client (started with `-C`/`-CC`, as iTerm2 and other front
//! ends do) does not draw the terminal itself; it speaks the textual control
//! protocol and lets the outer application render. Such a client behaves very
//! differently from a normal attached terminal — output is line-wrapped
//! notifications, not screen writes — so when something looks wrong it matters to
//! know which clients are control-mode. Where [`super::who`] and
//! [`super::connected`] list attached clients generally, `control` narrows to the
//! control-mode ones. Normal (screen-drawing) clients are omitted. With
//! `-o json` / `--json` it emits the same rows as a machine-readable array; a
//! server with no control-mode client prints just the header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-client format the fields are read through.
const FORMAT: &str =
    "#{client_name}\u{1f}#{client_session}\u{1f}#{client_tty}\u{1f}#{client_control_mode}";

/// One output row: a control-mode client.
struct Row {
    client: String,
    session: String,
    tty: String,
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

/// Parse one formatted line into `(client, session, tty, control)`.
fn parse_line(line: &str) -> Option<(String, String, String, bool)> {
    let mut it = line.split('\u{1f}');
    let client = it.next()?;
    let session = it.next()?;
    let tty = it.next()?;
    let control = it.next()? == "1";
    Some((
        client.to_string(),
        session.to_string(),
        tty.to_string(),
        control,
    ))
}

/// One row per control-mode client, ordered by client name.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, _, _, control)| *control)
        .map(|(client, session, tty, _)| Row {
            client,
            session,
            tty,
        })
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
        paint(
            &format!("{:<12} {:<12} {}", "CLIENT", "SESSION", "TTY"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!("{:<12} {:<12} {}\n", r.client, r.session, r.tty));
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
                "tty": r.tty,
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
        let (c, s, t, ctl) = parse_line("c0\u{1f}work\u{1f}/dev/ttys001\u{1f}1").unwrap();
        assert_eq!(c, "c0");
        assert_eq!(s, "work");
        assert_eq!(t, "/dev/ttys001");
        assert!(ctl);
    }

    #[test]
    fn normal_clients_are_omitted() {
        let lines = vec![
            "c0\u{1f}work\u{1f}/dev/ttys001\u{1f}0".to_string(),
            "c1\u{1f}work\u{1f}/dev/ttys002\u{1f}1".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].client, "c1");
    }

    #[test]
    fn none_control_renders_header_only() {
        let lines = vec!["c0\u{1f}s\u{1f}/dev/ttys001\u{1f}0".to_string()];
        let rows = build_rows(&lines);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("CLIENT") && s.contains("TTY"));
    }

    #[test]
    fn json_carries_client() {
        let lines = vec!["c0\u{1f}work\u{1f}/dev/ttys001\u{1f}1".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["client"], "c0");
        assert_eq!(v[0]["session"], "work");
    }
}
