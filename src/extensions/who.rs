//! `ztmux who` — every client attached to the server, grouped by session.
//!
//! Where [`super::active`] reports the focused window/pane *inside* each session
//! and [`super::stats`] rolls the server up into aggregate counts, `who` looks
//! from the other side of the socket: the live clients — the terminals actually
//! attached right now — with the tty they are on, the session each is viewing,
//! the client's screen size, its `$TERM`, and its pid. It is the "who else is
//! looking at this server, and from where" view (a shared/remote socket may
//! carry several). Clients are ordered by the session they view, then by tty, so
//! everyone watching one session groups together. With `-o json` / `--json` it
//! emits the same rows as a machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, poll};

/// One output row: a single attached client.
struct Row {
    name: String,
    tty: String,
    session: String,
    width: i64,
    height: i64,
    term: String,
    pid: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux who: {e}");
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

/// One row per attached client, grouped by the session it views (then tty) so
/// the clients watching the same session sort together; the client name breaks
/// any remaining tie for a stable order.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .clients
        .iter()
        .map(|c| Row {
            name: c.name.clone(),
            tty: c.tty.clone(),
            session: c.session.clone(),
            width: c.width,
            height: c.height,
            term: c.termname.clone(),
            pid: c.pid,
        })
        .collect();
    rows.sort_by(|a, b| {
        a.session
            .cmp(&b.session)
            .then(a.tty.cmp(&b.tty))
            .then(a.name.cmp(&b.name))
    });
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
            &format!(
                "{:<14} {:<16} {:<12} {:>9} {:<12} {:>7}",
                "CLIENT", "TTY", "SESSION", "SIZE", "TERM", "PID"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<14} {:<16} {:<12} {:>9} {:<12} {:>7}\n",
            r.name,
            r.tty,
            r.session,
            format!("{}x{}", r.width, r.height),
            r.term,
            r.pid,
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "name": r.name,
                "tty": r.tty,
                "session": r.session,
                "width": r.width,
                "height": r.height,
                "term": r.term,
                "pid": r.pid,
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

    fn client(name: &str, tty: &str, sess: &str, w: i64, h: i64, term: &str, pid: i64) -> Client {
        Client {
            name: name.into(),
            tty: tty.into(),
            session: sess.into(),
            width: w,
            height: h,
            termname: term.into(),
            pid,
        }
    }

    fn snap(clients: Vec<Client>) -> Snapshot {
        Snapshot {
            clients,
            ..Default::default()
        }
    }

    #[test]
    fn clients_group_by_session_then_tty() {
        let rows = build_rows(&snap(vec![
            client("c3", "/dev/ttys003", "work", 80, 24, "xterm", 30),
            client("c1", "/dev/ttys001", "work", 80, 24, "xterm", 10),
            client("c2", "/dev/ttys002", "admin", 80, 24, "xterm", 20),
        ]));
        // "admin" session sorts before "work".
        assert_eq!(rows[0].session, "admin");
        // Within "work", tty ttys001 precedes ttys003.
        assert_eq!(rows[1].tty, "/dev/ttys001");
        assert_eq!(rows[2].tty, "/dev/ttys003");
    }

    #[test]
    fn no_clients_renders_header_only() {
        let rows = build_rows(&snap(vec![]));
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("CLIENT") && s.contains("PID"));
    }

    #[test]
    fn text_renders_size_as_wxh() {
        let rows = build_rows(&snap(vec![client(
            "c1",
            "/dev/ttys001",
            "work",
            211,
            51,
            "screen-256color",
            99,
        )]));
        let s = render_text(&rows, false);
        assert!(
            s.lines()
                .any(|l| l.contains("211x51") && l.contains("screen-256color"))
        );
    }

    #[test]
    fn json_carries_all_client_fields() {
        let rows = build_rows(&snap(vec![client(
            "c1",
            "/dev/ttys001",
            "work",
            80,
            24,
            "xterm",
            42,
        )]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["name"], "c1");
        assert_eq!(v[0]["tty"], "/dev/ttys001");
        assert_eq!(v[0]["session"], "work");
        assert_eq!(v[0]["width"], 80);
        assert_eq!(v[0]["term"], "xterm");
        assert_eq!(v[0]["pid"], 42);
    }
}
