//! `ztmux term` — a histogram of the terminal types attached to the server.
//!
//! Where [`super::who`] lists each attached client individually, `term`
//! aggregates them by `$TERM`: how many clients are on `screen-256color`, how
//! many on `xterm-256color`, and so on. It is the client-side companion to
//! [`super::cmd`] (a histogram of pane commands) — the "what terminals is this
//! server being driven from" view, useful when a capability bug only shows up on
//! one `$TERM` and you need to know who is on it. Ordered most-common first.
//! With `-o json` / `--json` it emits the same rows as a machine-readable array;
//! a server with no clients prints just the header.

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, poll};

/// One output row: a terminal type and the clients using it.
struct Row {
    term: String,
    count: usize,
    clients: Vec<String>,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux term: {e}");
        return 1;
    }
    let rows = build_rows(&snap);
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

/// Count attached clients per `$TERM`, most-common first. A
/// [`std::collections::BTreeMap`] gives a deterministic term order; a stable
/// count-descending sort then puts the most common on top. Clients with an empty
/// `termname` are skipped.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in &snap.clients {
        if c.termname.is_empty() {
            continue;
        }
        buckets
            .entry(c.termname.clone())
            .or_default()
            .push(c.name.clone());
    }
    let mut rows: Vec<Row> = buckets
        .into_iter()
        .map(|(term, mut clients)| {
            clients.sort();
            Row {
                term,
                count: clients.len(),
                clients,
            }
        })
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.count));
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
        paint(&format!("{:>7} {:<24} {}", "CLIENTS", "TERM", "NAMES"), "1")
    ));
    for r in rows {
        out.push_str(&format!(
            "{:>7} {:<24} {}\n",
            r.count,
            r.term,
            r.clients.join(" "),
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "term": r.term,
                "clients": r.count,
                "names": r.clients,
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
    use super::super::tmux_query::Client;
    use super::*;

    fn client(name: &str, term: &str) -> Client {
        Client {
            name: name.into(),
            termname: term.into(),
            ..Default::default()
        }
    }

    fn snap(clients: Vec<Client>) -> Snapshot {
        Snapshot {
            clients,
            ..Default::default()
        }
    }

    #[test]
    fn most_common_term_sorts_first() {
        let rows = build_rows(&snap(vec![
            client("c1", "xterm-256color"),
            client("c2", "screen-256color"),
            client("c3", "screen-256color"),
        ]));
        assert_eq!(rows[0].term, "screen-256color");
        assert_eq!(rows[0].count, 2);
        assert_eq!(rows[1].term, "xterm-256color");
    }

    #[test]
    fn client_names_are_collected_and_sorted() {
        let rows = build_rows(&snap(vec![
            client("cb", "xterm"),
            client("ca", "xterm"),
        ]));
        assert_eq!(rows[0].clients, vec!["ca", "cb"]);
    }

    #[test]
    fn empty_term_is_skipped() {
        let rows = build_rows(&snap(vec![client("c1", ""), client("c2", "xterm")]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].term, "xterm");
    }

    #[test]
    fn no_clients_renders_header_only() {
        let rows = build_rows(&snap(vec![]));
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("TERM") && s.contains("CLIENTS"));
    }

    #[test]
    fn json_carries_term_count_and_names() {
        let rows = build_rows(&snap(vec![
            client("c1", "xterm"),
            client("c2", "xterm"),
        ]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["term"], "xterm");
        assert_eq!(v[0]["clients"], 2);
        assert_eq!(v[0]["names"][0], "c1");
    }
}
