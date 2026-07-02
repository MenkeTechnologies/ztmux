//! `ztmux net` — the established outbound connections of every pane.
//!
//! One `lsof` call lists every established TCP connection; each owning pid is
//! mapped back to the pane it runs under by walking the process tree (like
//! [`super::ports`] does for listening sockets). Where `ports` answers "what is
//! this pane serving" (inbound listeners) and [`super::ssh`] singles out SSH,
//! `net` answers "what is this pane talking to" — every live remote peer, for
//! any protocol. Connections under no pane are dropped, so the table stays the
//! panes' own traffic rather than the whole machine's. It degrades quietly when
//! `lsof` is missing. Output is a table (coloured on a TTY) or a JSON array with
//! `-o json` / `--json`, sorted by location then peer.

use std::io::IsTerminal;
use std::process::Command;

use super::proctree::{Proc, ancestor_in, basename, parent_map, table};
use super::tmux_query::{Pane, poll};

/// One established connection attributed to a pane.
struct Row {
    peer: String,
    pid: i64,
    command: String,
    pane: String,
    loc: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux net: {e}");
        return 1;
    }
    let procs = table();
    let conns = gather_conns();
    let rows = attribute(&conns, &procs, &snap.panes);
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

/// One `lsof` call for all established TCP connections; parse into `(pid, peer)`
/// pairs. Empty on any failure so the extension degrades to an empty table.
fn gather_conns() -> Vec<(i64, String)> {
    let Ok(out) = Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:ESTABLISHED", "-Fpn"])
        .output()
    else {
        return Vec::new();
    };
    parse_lsof(&String::from_utf8_lossy(&out.stdout))
}

/// Parse `lsof -Fpn` field output: `p<pid>` opens a process block, each following
/// `n<local>-><remote>` is one of its connections. The remote endpoint (after
/// `->`) is kept. Deduplicated by `(pid, peer)`.
fn parse_lsof(text: &str) -> Vec<(i64, String)> {
    let mut out: Vec<(i64, String)> = Vec::new();
    let mut pid: Option<i64> = None;
    for line in text.lines() {
        let Some((tag, rest)) = line.split_at_checked(1) else {
            continue;
        };
        match tag {
            "p" => pid = rest.parse().ok(),
            "n" => {
                if let (Some(pid), Some(peer)) = (pid, remote_of(rest)) {
                    let pair = (pid, peer);
                    if !out.contains(&pair) {
                        out.push(pair);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The remote endpoint of an `lsof` connection name (`laddr->raddr`): the part
/// after `->`. `None` when there is no arrow (e.g. a bare listening socket).
fn remote_of(name: &str) -> Option<String> {
    name.split_once("->").map(|(_, r)| r.to_string())
}

/// Attribute each connection to the pane whose pid owns it (directly or as an
/// ancestor). Connections under no pane are dropped. Sorted by location, peer.
fn attribute(conns: &[(i64, String)], procs: &[Proc], panes: &[Pane]) -> Vec<Row> {
    let pane_pids: Vec<i64> = panes.iter().map(|p| p.pid).collect();
    let pm = parent_map(procs);
    let comm: std::collections::HashMap<i64, &str> =
        procs.iter().map(|p| (p.pid, p.comm.as_str())).collect();

    let mut rows: Vec<Row> = conns
        .iter()
        .filter_map(|(pid, peer)| {
            let owner = if pane_pids.contains(pid) {
                Some(*pid)
            } else {
                ancestor_in(*pid, &pm, &pane_pids)
            }?;
            let pane = panes.iter().find(|p| p.pid == owner)?;
            Some(Row {
                peer: peer.clone(),
                pid: *pid,
                command: comm
                    .get(pid)
                    .map_or_else(|| "?".to_string(), |c| basename(c).to_string()),
                pane: pane.id.clone(),
                loc: format!("{}:{}.{}", pane.session, pane.window, pane.index),
            })
        })
        .collect();
    rows.sort_by(|a, b| a.loc.cmp(&b.loc).then(a.peer.cmp(&b.peer)));
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
                "{:<8} {:<16} {:>7} {:<12} {}",
                "PANE", "LOCATION", "PID", "COMMAND", "PEER"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:>7} {:<12} {}\n",
            r.pane, r.loc, r.pid, r.command, r.peer,
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "pane": r.pane,
                "location": r.loc,
                "pid": r.pid,
                "command": r.command,
                "peer": r.peer,
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
    fn remote_of_takes_the_far_endpoint() {
        assert_eq!(
            remote_of("127.0.0.1:5000->1.2.3.4:443").as_deref(),
            Some("1.2.3.4:443")
        );
        assert_eq!(
            remote_of("[::1]:5000->[2606::1]:443").as_deref(),
            Some("[2606::1]:443")
        );
        assert_eq!(remote_of("*:7000"), None); // no arrow (listener)
    }

    #[test]
    fn parse_lsof_pairs_pid_with_remote_and_dedupes() {
        let text = "p100\nn10.0.0.2:50->1.1.1.1:443\nn10.0.0.2:50->1.1.1.1:443\n\
                    p200\nn10.0.0.2:51->8.8.8.8:53\n";
        let got = parse_lsof(text);
        assert_eq!(
            got,
            vec![
                (100, "1.1.1.1:443".to_string()),
                (200, "8.8.8.8:53".to_string()),
            ]
        );
    }

    fn procs() -> Vec<Proc> {
        vec![
            Proc {
                pid: 100,
                ppid: 1,
                comm: "zsh".into(),
            },
            Proc {
                pid: 300,
                ppid: 100,
                comm: "curl".into(),
            },
        ]
    }

    fn panes() -> Vec<Pane> {
        vec![Pane {
            id: "%0".into(),
            session: "net".into(),
            window: 1,
            index: 0,
            pid: 100,
            ..Default::default()
        }]
    }

    #[test]
    fn attributes_a_descendant_connection_to_its_pane() {
        // curl (300) connects; its pane is the zsh (100) above it.
        let rows = attribute(&[(300, "1.1.1.1:443".into())], &procs(), &panes());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pane, "%0");
        assert_eq!(rows[0].loc, "net:1.0");
        assert_eq!(rows[0].command, "curl");
        assert_eq!(rows[0].peer, "1.1.1.1:443");
    }

    #[test]
    fn connection_under_no_pane_is_dropped() {
        let rows = attribute(&[(999, "9.9.9.9:443".into())], &procs(), &panes());
        assert!(rows.is_empty());
    }

    #[test]
    fn rows_sorted_by_location_then_peer() {
        let panes = vec![
            Pane {
                id: "%0".into(),
                session: "a".into(),
                window: 0,
                index: 0,
                pid: 100,
                ..Default::default()
            },
            Pane {
                id: "%1".into(),
                session: "a".into(),
                window: 1,
                index: 0,
                pid: 200,
                ..Default::default()
            },
        ];
        let procs = vec![
            Proc {
                pid: 100,
                ppid: 1,
                comm: "zsh".into(),
            },
            Proc {
                pid: 200,
                ppid: 1,
                comm: "zsh".into(),
            },
        ];
        let rows = attribute(
            &[
                (200, "2.2.2.2:80".into()),
                (100, "3.3.3.3:80".into()),
                (100, "1.1.1.1:80".into()),
            ],
            &procs,
            &panes,
        );
        assert_eq!(
            rows.iter()
                .map(|r| (r.loc.as_str(), r.peer.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("a:0.0", "1.1.1.1:80"),
                ("a:0.0", "3.3.3.3:80"),
                ("a:1.0", "2.2.2.2:80"),
            ],
        );
    }

    #[test]
    fn text_has_header_and_a_row() {
        let rows = attribute(&[(300, "1.1.1.1:443".into())], &procs(), &panes());
        let s = render_text(&rows, false);
        assert!(s.contains("PEER") && s.contains("COMMAND"));
        assert!(s.contains("1.1.1.1:443") && s.contains("%0") && s.contains("curl"));
    }
}
