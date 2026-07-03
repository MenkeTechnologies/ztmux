//! `ztmux utf8` — the UTF-8 state of every attached client.
//!
//! tmux decides per client whether the client's terminal is UTF-8 capable
//! (`#{client_utf8}`), derived from `LANG`/`LC_*`, the `-u` flag, and the
//! terminal's advertised features. A client that tmux thinks is *not* UTF-8 will
//! draw wide/box-drawing/emoji glyphs as `?`/mojibake even though every other
//! client renders them fine — the classic "it's broken only on that one
//! terminal" bug. Unlike the filtered flag reports ([`super::visual`],
//! [`super::keytable`]), `utf8` lists *all* clients with a `yes`/`no` column,
//! because both states are diagnostic. Where [`super::term`] buckets clients by
//! `$TERM`, `utf8` answers "does tmux consider this client Unicode-clean".
//! Ordered by client name. With `-o json` / `--json` it emits the same rows as a
//! machine-readable array; a server with no clients prints just the header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-client format the fields are read through.
const FORMAT: &str =
    "#{client_name}\u{1f}#{client_session}\u{1f}#{client_tty}\u{1f}#{client_utf8}";

/// One output row: a client and whether tmux treats it as UTF-8.
struct Row {
    client: String,
    session: String,
    tty: String,
    utf8: bool,
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

/// Parse one formatted line into `(client, session, tty, utf8)`.
fn parse_line(line: &str) -> Option<(String, String, String, bool)> {
    let mut it = line.split('\u{1f}');
    let client = it.next()?;
    let session = it.next()?;
    let tty = it.next()?;
    let utf8 = it.next()? == "1";
    Some((
        client.to_string(),
        session.to_string(),
        tty.to_string(),
        utf8,
    ))
}

/// One row per client (UTF-8 or not), ordered by client name.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .map(|(client, session, tty, utf8)| Row {
            client,
            session,
            tty,
            utf8,
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
    let yn = |b: bool| if b { "yes" } else { "no" };
    let mut out = String::new();
    out.push_str(&format!(
        "{}\n",
        paint(
            &format!("{:<12} {:<12} {:<14} {}", "CLIENT", "SESSION", "TTY", "UTF8"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<12} {:<12} {:<14} {}\n",
            r.client,
            r.session,
            r.tty,
            yn(r.utf8),
        ));
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
                "utf8": r.utf8,
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
        let (c, s, t, u) = parse_line("c0\u{1f}work\u{1f}/dev/ttys001\u{1f}1").unwrap();
        assert_eq!(c, "c0");
        assert_eq!(s, "work");
        assert_eq!(t, "/dev/ttys001");
        assert!(u);
    }

    #[test]
    fn both_utf8_states_are_listed() {
        let lines = vec![
            "c0\u{1f}work\u{1f}/dev/ttys001\u{1f}1".to_string(),
            "c1\u{1f}work\u{1f}/dev/ttys002\u{1f}0".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].utf8);
        assert!(!rows[1].utf8);
    }

    #[test]
    fn no_clients_renders_header_only() {
        let rows = build_rows(&[]);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("CLIENT") && s.contains("UTF8"));
    }

    #[test]
    fn json_carries_utf8_flag() {
        let lines = vec!["c0\u{1f}work\u{1f}/dev/ttys001\u{1f}0".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["client"], "c0");
        assert_eq!(v[0]["utf8"], false);
    }
}
