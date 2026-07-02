//! `ztmux ports` — which pane is listening on which TCP port.
//!
//! One `lsof` call lists every listening TCP socket; each owning pid is mapped
//! back to the pane it runs under by walking the process tree
//! (via [`super::proctree`]) until a pane's pid is reached. This answers "which
//! pane is my dev server running in" across the whole server. It degrades
//! quietly: if `lsof` is missing or returns nothing, the table is empty. Output
//! is a table (coloured when stdout is a TTY) or a JSON array with `-o json` /
//! `--json`. Rows are sorted by port.

use std::io::IsTerminal;
use std::process::Command;

use super::proctree::{Proc, ancestor_in, basename, parent_map, table};
use super::tmux_query::{Pane, poll};

/// One listening socket attributed to a pane (if one owns it).
struct Row {
    port: u32,
    pid: i64,
    command: String,
    pane: String,
    loc: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux ports: {e}");
        return 1;
    }
    let procs = table();
    let listens = gather_listens();
    let rows = attribute(&listens, &procs, &snap.panes);
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

/// One `lsof` call for all listening TCP sockets; parse into `(pid, port)`
/// pairs. Empty on any failure so the extension degrades to an empty table.
fn gather_listens() -> Vec<(i64, u32)> {
    let Ok(out) = Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-Fpn"])
        .output()
    else {
        return Vec::new();
    };
    parse_lsof(&String::from_utf8_lossy(&out.stdout))
}

/// Parse `lsof -Fpn` field output: `p<pid>` opens a process block, each
/// following `n<addr:port>` is one of its listening sockets. Deduplicated by
/// `(pid, port)` so IPv4+IPv6 duplicates collapse.
fn parse_lsof(text: &str) -> Vec<(i64, u32)> {
    let mut out = Vec::new();
    let mut pid: Option<i64> = None;
    for line in text.lines() {
        let Some((tag, rest)) = line.split_at_checked(1) else {
            continue;
        };
        match tag {
            "p" => pid = rest.parse().ok(),
            "n" => {
                if let (Some(pid), Some(port)) = (pid, port_of(rest)) {
                    let pair = (pid, port);
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

/// The port from an `lsof` name field: the number after the final `:`
/// (`*:7000` → 7000, `127.0.0.1:5000` → 5000, `[::1]:8080` → 8080). `None` for
/// wildcard/non-numeric ports (`*:*`).
fn port_of(name: &str) -> Option<u32> {
    name.rsplit(':').next()?.parse().ok()
}

/// Attribute each listening socket to the pane whose pid is the socket's owner
/// or an ancestor of it. Sockets with no owning pane are still listed with an
/// empty pane/`-` location. Sorted by port, then pane.
fn attribute(listens: &[(i64, u32)], procs: &[Proc], panes: &[Pane]) -> Vec<Row> {
    let pane_pids: Vec<i64> = panes.iter().map(|p| p.pid).collect();
    let pm = parent_map(procs);
    let comm: std::collections::HashMap<i64, &str> =
        procs.iter().map(|p| (p.pid, p.comm.as_str())).collect();

    let mut rows: Vec<Row> = listens
        .iter()
        .map(|&(pid, port)| {
            let owner = if pane_pids.contains(&pid) {
                Some(pid)
            } else {
                ancestor_in(pid, &pm, &pane_pids)
            };
            let (pane, loc) = owner
                .and_then(|op| panes.iter().find(|p| p.pid == op))
                .map_or((String::new(), "-".to_string()), |p| (p.id.clone(), loc(p)));
            Row {
                port,
                pid,
                command: comm
                    .get(&pid)
                    .map_or_else(|| "?".to_string(), |c| basename(c).to_string()),
                pane,
                loc,
            }
        })
        .collect();
    rows.sort_by(|a, b| a.port.cmp(&b.port).then(a.pane.cmp(&b.pane)));
    rows
}

fn loc(p: &Pane) -> String {
    format!("{}:{}.{}", p.session, p.window, p.index)
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
                "{:>6} {:<8} {:<16} {:>7} {}",
                "PORT", "PANE", "LOCATION", "PID", "COMMAND"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:>6} {:<8} {:<16} {:>7} {}\n",
            r.port,
            if r.pane.is_empty() { "-" } else { &r.pane },
            r.loc,
            r.pid,
            r.command,
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "port": r.port,
                "pane": r.pane,
                "location": r.loc,
                "pid": r.pid,
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
    fn parse_lsof_dedupes_ipv4_and_ipv6() {
        let text = "p31734\nn*:7000\nn*:7000\nn*:5000\np31788\nn127.0.0.1:8080\n";
        let got = parse_lsof(text);
        assert_eq!(got, vec![(31734, 7000), (31734, 5000), (31788, 8080)]);
    }

    #[test]
    fn parse_lsof_skips_wildcard_port() {
        assert!(parse_lsof("p10\nn*:*\n").is_empty());
    }

    #[test]
    fn port_of_handles_ipv4_ipv6_and_wildcard() {
        assert_eq!(port_of("*:7000"), Some(7000));
        assert_eq!(port_of("127.0.0.1:5000"), Some(5000));
        assert_eq!(port_of("[::1]:8080"), Some(8080));
        assert_eq!(port_of("*:*"), None);
    }

    fn procs() -> Vec<Proc> {
        vec![
            Proc {
                pid: 100,
                ppid: 1,
                comm: "zsh".into(),
            },
            Proc {
                pid: 200,
                ppid: 100,
                comm: "npm".into(),
            },
            Proc {
                pid: 300,
                ppid: 200,
                comm: "node".into(),
            },
        ]
    }

    fn panes() -> Vec<Pane> {
        vec![Pane {
            id: "%0".into(),
            session: "web".into(),
            window: 0,
            index: 0,
            pid: 100,
            ..Default::default()
        }]
    }

    #[test]
    fn attributes_a_descendant_socket_to_its_pane() {
        // node (300) listens; its pane is the zsh (100) two levels up.
        let rows = attribute(&[(300, 3000)], &procs(), &panes());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].port, 3000);
        assert_eq!(rows[0].pane, "%0");
        assert_eq!(rows[0].loc, "web:0.0");
        assert_eq!(rows[0].command, "node");
    }

    #[test]
    fn socket_owned_directly_by_the_pane_pid_attributes() {
        let rows = attribute(&[(100, 22)], &procs(), &panes());
        assert_eq!(rows[0].pane, "%0");
        assert_eq!(rows[0].command, "zsh");
    }

    #[test]
    fn unowned_socket_listed_with_empty_pane() {
        // pid 999 is unknown / not under any pane.
        let rows = attribute(&[(999, 443)], &procs(), &panes());
        assert_eq!(rows[0].pane, "");
        assert_eq!(rows[0].loc, "-");
        assert_eq!(rows[0].command, "?");
    }

    #[test]
    fn rows_sorted_by_port() {
        let rows = attribute(&[(300, 8080), (200, 3000), (100, 22)], &procs(), &panes());
        let ports: Vec<u32> = rows.iter().map(|r| r.port).collect();
        assert_eq!(ports, vec![22, 3000, 8080]);
    }

    #[test]
    fn text_has_header_and_a_row() {
        let rows = attribute(&[(300, 3000)], &procs(), &panes());
        let s = render_text(&rows, false);
        assert!(s.contains("PORT") && s.contains("COMMAND"));
        assert!(s.contains("3000") && s.contains("%0") && s.contains("node"));
    }
}
