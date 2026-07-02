//! `ztmux fanout` — sessions ranked by how many panes they hold in total.
//!
//! Where [`super::density`] ranks individual *windows* by pane count and
//! [`super::usage`] ranks sessions by the CPU/MEM they burn (reading `/proc`),
//! `fanout` ranks *sessions* by their total pane count across every window —
//! pure structure, no resource probing. It answers "which session have I fanned
//! out the most panes in", a fast, always-available structural size (a session
//! full of idle shells is large here yet light in `usage`). The pane total is
//! summed from each window's pane count. With `-o json` / `--json` it emits the
//! same rows as a machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, poll};

/// One output row: a session with its window and total-pane counts.
struct Row {
    name: String,
    windows: i64,
    panes: i64,
    attached: bool,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux fanout: {e}");
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

/// One row per session, most total panes first; ties break by name for a stable
/// order. The pane total is the sum of every window's pane count in that session.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    use std::collections::BTreeMap;
    let mut panes_by_session: BTreeMap<&str, i64> = BTreeMap::new();
    for w in &snap.windows {
        *panes_by_session.entry(w.session.as_str()).or_default() += w.panes;
    }
    let mut rows: Vec<Row> = snap
        .sessions
        .iter()
        .map(|s| Row {
            name: s.name.clone(),
            windows: s.windows,
            panes: panes_by_session.get(s.name.as_str()).copied().unwrap_or(0),
            attached: s.attached,
        })
        .collect();
    rows.sort_by(|a, b| b.panes.cmp(&a.panes).then(a.name.cmp(&b.name)));
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
                "{:>6} {:>7} {:>8} {}",
                "PANES", "WINDOWS", "ATTACHED", "SESSION"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:>6} {:>7} {:>8} {}\n",
            r.panes,
            r.windows,
            if r.attached { "yes" } else { "no" },
            r.name,
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
                "windows": r.windows,
                "panes": r.panes,
                "attached": r.attached,
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
    use super::super::tmux_query::{Session, Window};
    use super::*;

    fn session(name: &str, windows: i64, attached: bool) -> Session {
        Session {
            name: name.into(),
            windows,
            attached,
            ..Default::default()
        }
    }

    fn window(sess: &str, idx: i64, panes: i64) -> Window {
        Window {
            session: sess.into(),
            index: idx,
            panes,
            ..Default::default()
        }
    }

    fn snap(sessions: Vec<Session>, windows: Vec<Window>) -> Snapshot {
        Snapshot {
            sessions,
            windows,
            ..Default::default()
        }
    }

    #[test]
    fn sessions_sorted_by_total_pane_count() {
        let rows = build_rows(&snap(
            vec![session("small", 1, false), session("big", 2, true)],
            vec![
                window("small", 0, 1),
                window("big", 0, 3),
                window("big", 1, 4),
            ],
        ));
        assert_eq!(rows[0].name, "big");
        assert_eq!(rows[0].panes, 7);
        assert_eq!(rows[1].name, "small");
        assert_eq!(rows[1].panes, 1);
    }

    #[test]
    fn equal_totals_break_ties_by_name() {
        let rows = build_rows(&snap(
            vec![session("bbb", 1, false), session("aaa", 1, false)],
            vec![window("bbb", 0, 2), window("aaa", 0, 2)],
        ));
        assert_eq!(rows[0].name, "aaa");
        assert_eq!(rows[1].name, "bbb");
    }

    #[test]
    fn session_with_no_windows_totals_zero() {
        let rows = build_rows(&snap(vec![session("empty", 0, false)], vec![]));
        assert_eq!(rows[0].panes, 0);
    }

    #[test]
    fn text_renders_totals_and_session() {
        let rows = build_rows(&snap(
            vec![session("big", 2, true)],
            vec![window("big", 0, 3), window("big", 1, 4)],
        ));
        let s = render_text(&rows, false);
        assert!(s.contains("PANES") && s.contains("SESSION"));
        assert!(s.lines().any(|l| l.contains("big") && l.contains("yes")));
    }

    #[test]
    fn json_carries_totals() {
        let rows = build_rows(&snap(
            vec![session("big", 2, true)],
            vec![window("big", 0, 3), window("big", 1, 4)],
        ));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["name"], "big");
        assert_eq!(v[0]["windows"], 2);
        assert_eq!(v[0]["panes"], 7);
        assert_eq!(v[0]["attached"], true);
    }
}
