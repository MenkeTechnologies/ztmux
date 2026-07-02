//! `ztmux usage` — per-session resource rollup, busiest session first.
//!
//! Where [`super::ps`] lists one row per pane and [`super::stats`] reports
//! server-wide counts, this joins every pane's process stats (from
//! [`super::procstat`]) and aggregates CPU%, MEM%, and RSS by session — the
//! "which session is eating the machine" view. Output is a table (coloured when
//! stdout is a TTY) or a machine-readable array with `-o json` / `--json`. Rows
//! are sorted by total CPU descending.

use std::collections::BTreeMap;
use std::io::IsTerminal;

use super::procstat::{fmt_rss, gather};
use super::tmux_query::{Snapshot, poll};

/// One output row: a session with its pane/window counts and summed stats.
#[derive(Default)]
struct Usage {
    session: String,
    windows: usize,
    panes: usize,
    cpu: f32,
    mem: f32,
    rss_kb: u64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux usage: {e}");
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

fn build_rows(snap: &Snapshot) -> Vec<Usage> {
    let pids: Vec<i64> = snap
        .panes
        .iter()
        .map(|p| p.pid)
        .filter(|&p| p > 0)
        .collect();
    let stats = gather(&pids);

    // Window count per session comes straight from the window list; pane stats
    // are folded in below so a session with no live panes still appears.
    let mut by_session: BTreeMap<&str, Usage> = BTreeMap::new();
    for s in &snap.sessions {
        by_session
            .entry(s.name.as_str())
            .or_default()
            .session
            .clone_from(&s.name);
    }
    for w in &snap.windows {
        by_session.entry(w.session.as_str()).or_default().windows += 1;
    }
    for p in &snap.panes {
        let u = by_session.entry(p.session.as_str()).or_default();
        u.panes += 1;
        if let Some(st) = stats.get(&p.pid) {
            u.cpu += st.cpu;
            u.mem += st.mem;
            u.rss_kb += st.rss_kb;
        }
    }

    let mut rows: Vec<Usage> = by_session.into_values().collect();
    rows.sort_by(|a, b| {
        b.cpu
            .partial_cmp(&a.cpu)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.session.cmp(&b.session))
    });
    rows
}

fn render_text(rows: &[Usage], color: bool) -> String {
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
                "{:<20} {:>7} {:>6} {:>6} {:>6} {:>8}",
                "SESSION", "WINDOWS", "PANES", "%CPU", "%MEM", "RSS"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<20} {:>7} {:>6} {:>6.1} {:>6.1} {:>8}\n",
            r.session,
            r.windows,
            r.panes,
            r.cpu,
            r.mem,
            fmt_rss(r.rss_kb),
        ));
    }
    out
}

fn render_json(rows: &[Usage]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "session": r.session,
                "windows": r.windows,
                "panes": r.panes,
                "cpu": r.cpu,
                "mem": r.mem,
                "rss_kb": r.rss_kb,
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
    use super::super::tmux_query::{Pane, Session, Window};
    use super::*;

    fn snap() -> Snapshot {
        Snapshot {
            sessions: vec![
                Session {
                    name: "a".into(),
                    ..Default::default()
                },
                Session {
                    name: "b".into(),
                    ..Default::default()
                },
            ],
            windows: vec![
                Window {
                    session: "a".into(),
                    index: 0,
                    ..Default::default()
                },
                Window {
                    session: "a".into(),
                    index: 1,
                    ..Default::default()
                },
                Window {
                    session: "b".into(),
                    index: 0,
                    ..Default::default()
                },
            ],
            panes: vec![
                Pane {
                    session: "a".into(),
                    window: 0,
                    index: 0,
                    pid: 10,
                    ..Default::default()
                },
                Pane {
                    session: "a".into(),
                    window: 1,
                    index: 0,
                    pid: 11,
                    ..Default::default()
                },
                Pane {
                    session: "b".into(),
                    window: 0,
                    index: 0,
                    pid: 20,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn one_row_per_session_with_window_and_pane_counts() {
        let rows = build_rows(&snap());
        assert_eq!(rows.len(), 2);
        let a = rows.iter().find(|r| r.session == "a").unwrap();
        assert_eq!(a.windows, 2);
        assert_eq!(a.panes, 2);
        let b = rows.iter().find(|r| r.session == "b").unwrap();
        assert_eq!(b.windows, 1);
        assert_eq!(b.panes, 1);
    }

    // A session that exists but has no windows/panes still gets a zeroed row.
    #[test]
    fn empty_session_still_appears() {
        let mut sn = snap();
        sn.sessions.push(Session {
            name: "idle".into(),
            ..Default::default()
        });
        let rows = build_rows(&sn);
        let idle = rows.iter().find(|r| r.session == "idle").unwrap();
        assert_eq!(idle.windows, 0);
        assert_eq!(idle.panes, 0);
    }

    #[test]
    fn rows_sorted_by_cpu_descending() {
        // No live server in tests, so all cpu is 0; ties fall back to name asc.
        let rows = build_rows(&snap());
        assert_eq!(rows[0].session, "a");
        assert_eq!(rows[1].session, "b");
    }

    #[test]
    fn json_is_an_array_of_sessions() {
        let rows = build_rows(&snap());
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert!(v.is_array());
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert!(v[0].get("session").is_some());
        assert!(v[0].get("rss_kb").is_some());
    }

    #[test]
    fn text_has_header_and_a_row() {
        // Hand-build a row to exercise the summed-stat formatting.
        let rows = vec![Usage {
            session: "work".into(),
            windows: 3,
            panes: 7,
            cpu: 12.5,
            mem: 4.0,
            rss_kb: 2048,
        }];
        let s = render_text(&rows, false);
        assert!(s.contains("SESSION") && s.contains("%CPU"));
        assert!(s.contains("work") && s.contains("12.5") && s.contains("2.0M"));
    }
}
