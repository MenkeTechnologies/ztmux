//! `ztmux ps` — a one-shot, pipeable table of every pane's process.
//!
//! The non-interactive complement to the [`super::watch`] TUI: it joins the
//! pane list from the `list-* -o json` query layer with per-process CPU/memory
//! from [`super::procstat`] and prints a table (coloured when stdout is a TTY),
//! or a machine-readable array with `-o json` / `--json`. Rows are sorted by
//! CPU descending.

use std::io::IsTerminal;

use super::procstat::{ProcStat, fmt_rss, gather};
use super::tmux_query::{Snapshot, poll};

/// One output row: pane identity joined with its process stats.
struct PsRow {
    id: String,
    loc: String,
    command: String,
    pid: i64,
    stat: ProcStat,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux ps: {e}");
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

fn build_rows(snap: &Snapshot) -> Vec<PsRow> {
    let pids: Vec<i64> = snap
        .panes
        .iter()
        .map(|p| p.pid)
        .filter(|&p| p > 0)
        .collect();
    let stats = gather(&pids);
    let mut rows: Vec<PsRow> = snap
        .panes
        .iter()
        .map(|p| PsRow {
            id: p.id.clone(),
            loc: format!("{}:{}.{}", p.session, p.window, p.index),
            command: p.command.clone(),
            pid: p.pid,
            stat: stats.get(&p.pid).cloned().unwrap_or_default(),
        })
        .collect();
    rows.sort_by(|a, b| {
        b.stat
            .cpu
            .partial_cmp(&a.stat.cpu)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows
}

fn render_text(rows: &[PsRow], color: bool) -> String {
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
                "{:<8} {:<16} {:<16} {:>7} {:>6} {:>6} {:>8} {:>3}",
                "PANE", "LOCATION", "COMMAND", "PID", "%CPU", "%MEM", "RSS", "ST"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<16} {:>7} {:>6.1} {:>6.1} {:>8} {:>3}\n",
            r.id,
            r.loc,
            r.command,
            r.pid,
            r.stat.cpu,
            r.stat.mem,
            fmt_rss(r.stat.rss_kb),
            r.stat.state,
        ));
    }
    out
}

fn render_json(rows: &[PsRow]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "pane": r.id,
                "location": r.loc,
                "command": r.command,
                "pid": r.pid,
                "cpu": r.stat.cpu,
                "mem": r.stat.mem,
                "rss_kb": r.stat.rss_kb,
                "state": r.stat.state,
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
    use super::super::tmux_query::Pane;
    use super::*;
    use std::collections::HashMap;

    fn snap() -> Snapshot {
        Snapshot {
            panes: vec![
                Pane {
                    id: "%0".into(),
                    session: "w".into(),
                    window: 0,
                    index: 0,
                    pid: 10,
                    command: "zsh".into(),
                    ..Default::default()
                },
                Pane {
                    id: "%1".into(),
                    session: "w".into(),
                    window: 0,
                    index: 1,
                    pid: 20,
                    command: "nvim".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn rows_built_for_every_pane() {
        // No live server in tests, so stats are absent (zeroed) but rows exist.
        let rows = build_rows(&snap());
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.command == "nvim"));
    }

    #[test]
    fn json_is_an_array_of_panes() {
        let rows = build_rows(&snap());
        let out = render_json(&rows);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.is_array());
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert!(v[0].get("pane").is_some());
    }

    #[test]
    fn text_has_a_header_and_a_row() {
        let mut stats = HashMap::new();
        stats.insert(
            10,
            ProcStat {
                cpu: 3.0,
                mem: 1.0,
                rss_kb: 2048,
                state: "S".into(),
            },
        );
        // build_rows can't inject stats (it calls ps), so render a hand-built row.
        let rows = vec![PsRow {
            id: "%0".into(),
            loc: "w:0.0".into(),
            command: "zsh".into(),
            pid: 10,
            stat: stats.remove(&10).unwrap(),
        }];
        let s = render_text(&rows, false);
        assert!(s.contains("PANE") && s.contains("COMMAND"));
        assert!(s.contains("zsh") && s.contains("2.0M"));
    }

    // Each JSON row carries the pane identity and the joined process stats.
    #[test]
    fn json_row_carries_all_fields() {
        let rows = vec![PsRow {
            id: "%5".into(),
            loc: "w:1.0".into(),
            command: "top".into(),
            pid: 42,
            stat: ProcStat {
                cpu: 7.5,
                mem: 2.0,
                rss_kb: 4096,
                state: "R".into(),
            },
        }];
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["pane"], "%5");
        assert_eq!(v[0]["location"], "w:1.0");
        assert_eq!(v[0]["pid"], 42);
        assert_eq!(v[0]["rss_kb"], 4096);
        assert_eq!(v[0]["state"], "R");
    }

    // build_rows composes the location as session:window.index and preserves pid.
    #[test]
    fn rows_location_format_is_session_window_pane() {
        let rows = build_rows(&snap());
        let r = rows.iter().find(|r| r.id == "%1").unwrap();
        assert_eq!(r.loc, "w:0.1");
        assert_eq!(r.pid, 20);
    }
}
