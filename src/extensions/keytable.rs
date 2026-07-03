//! `ztmux keytable` — clients that are parked on a non-root key table.
//!
//! Every client dispatches keys through a key table; normally that is `root`,
//! but pressing the prefix switches it to `prefix`, entering copy mode switches
//! it to `copy-mode`/`copy-mode-vi`, and `switch-client -T <table>` /
//! `bind -T <table>` can leave a client on any custom table. A client stuck on a
//! non-root table is why keys "stop working" (they are being interpreted by the
//! wrong table), so this lists exactly those clients and which table they are on.
//! Where [`super::keys`] lists the *bindings* in the tables, `keytable` lists the
//! *clients* currently in them. Clients on `root` are omitted. With `-o json` /
//! `--json` it emits the same rows as a machine-readable array; a server whose
//! clients are all on `root` prints just the header.

use std::io::IsTerminal;

use super::tmux_query::query_lines;

/// The `\x1f`-delimited per-client format the fields are read through.
const FORMAT: &str =
    "#{client_name}\u{1f}#{client_session}\u{1f}#{client_tty}\u{1f}#{client_key_table}";

/// One output row: a client and the non-root key table it is on.
struct Row {
    client: String,
    session: String,
    tty: String,
    table: String,
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

/// Parse one formatted line into `(client, session, tty, table)`.
fn parse_line(line: &str) -> Option<(String, String, String, String)> {
    let mut it = line.split('\u{1f}');
    let client = it.next()?;
    let session = it.next()?;
    let tty = it.next()?;
    let table = it.next()?;
    Some((
        client.to_string(),
        session.to_string(),
        tty.to_string(),
        table.to_string(),
    ))
}

/// One row per client whose key table is not `root`, ordered by client name.
fn build_rows(lines: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .filter_map(|l| parse_line(l))
        .filter(|(_, _, _, table)| table != "root")
        .map(|(client, session, tty, table)| Row {
            client,
            session,
            tty,
            table,
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
            &format!("{:<12} {:<12} {:<14} {}", "CLIENT", "SESSION", "TTY", "TABLE"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<12} {:<12} {:<14} {}\n",
            r.client, r.session, r.tty, r.table,
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
                "table": r.table,
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
        let (c, s, t, tbl) =
            parse_line("c0\u{1f}work\u{1f}/dev/ttys001\u{1f}copy-mode-vi").unwrap();
        assert_eq!(c, "c0");
        assert_eq!(s, "work");
        assert_eq!(t, "/dev/ttys001");
        assert_eq!(tbl, "copy-mode-vi");
    }

    #[test]
    fn root_table_clients_are_omitted() {
        let lines = vec![
            "c0\u{1f}work\u{1f}/dev/ttys001\u{1f}root".to_string(),
            "c1\u{1f}work\u{1f}/dev/ttys002\u{1f}prefix".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].client, "c1");
        assert_eq!(rows[0].table, "prefix");
    }

    #[test]
    fn all_root_renders_header_only() {
        let lines = vec!["c0\u{1f}s\u{1f}/dev/ttys001\u{1f}root".to_string()];
        let rows = build_rows(&lines);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("CLIENT") && s.contains("TABLE"));
    }

    #[test]
    fn rows_sort_by_client_name() {
        let lines = vec![
            "cb\u{1f}s\u{1f}/dev/ttys002\u{1f}prefix".to_string(),
            "ca\u{1f}s\u{1f}/dev/ttys001\u{1f}copy-mode".to_string(),
        ];
        let rows = build_rows(&lines);
        assert_eq!(rows[0].client, "ca");
        assert_eq!(rows[1].client, "cb");
    }

    #[test]
    fn json_carries_table() {
        let lines = vec!["c0\u{1f}work\u{1f}/dev/ttys001\u{1f}prefix".to_string()];
        let rows = build_rows(&lines);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["client"], "c0");
        assert_eq!(v[0]["table"], "prefix");
    }
}
