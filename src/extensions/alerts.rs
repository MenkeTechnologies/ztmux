//! `ztmux alerts` — windows with a pending bell, activity, or silence alert.
//!
//! tmux raises a window alert when a background window rings the bell, produces
//! output while `monitor-activity` is on, or goes quiet while `monitor-silence`
//! is on — normally shown only as a flag in the status line of an attached
//! client. `alerts` lists every window currently flagged and which alerts are
//! pending, across the whole server, so the window that just beeped or finished
//! is one command away without an attached client. Windows with no pending alert
//! are omitted. Sorted by session then window. With `-o json` / `--json` it emits
//! the same rows as a machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, Window, poll};

/// One output row: a flagged window and which alerts are pending on it.
struct Row {
    session: String,
    window: i64,
    window_name: String,
    alerts: Vec<String>,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux alerts: {e}");
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

/// The alert types currently pending on a window, in a stable order.
fn alert_types(w: &Window) -> Vec<String> {
    let mut v = Vec::new();
    if w.bell {
        v.push("bell".to_string());
    }
    if w.activity {
        v.push("activity".to_string());
    }
    if w.silence {
        v.push("silence".to_string());
    }
    v
}

/// One row per window that has at least one pending alert, ordered by session
/// then window index.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .windows
        .iter()
        .filter_map(|w| {
            let alerts = alert_types(w);
            (!alerts.is_empty()).then(|| Row {
                session: w.session.clone(),
                window: w.index,
                window_name: w.name.clone(),
                alerts,
            })
        })
        .collect();
    rows.sort_by(|a, b| a.session.cmp(&b.session).then(a.window.cmp(&b.window)));
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
            &format!("{:<20} {:<16} {}", "SESSION", "WINDOW", "ALERTS"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<20} {:<16} {}\n",
            r.session,
            format!("{}:{}", r.window, r.window_name),
            r.alerts.join(","),
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "session": r.session,
                "window": r.window,
                "window_name": r.window_name,
                "alerts": r.alerts,
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

    fn win(
        session: &str,
        index: i64,
        name: &str,
        bell: bool,
        activity: bool,
        silence: bool,
    ) -> Window {
        Window {
            session: session.into(),
            index,
            name: name.into(),
            bell,
            activity,
            silence,
            ..Default::default()
        }
    }

    fn snap(windows: Vec<Window>) -> Snapshot {
        Snapshot {
            windows,
            ..Default::default()
        }
    }

    #[test]
    fn alert_types_lists_active_flags_in_order() {
        assert_eq!(
            alert_types(&win("a", 0, "w", true, false, true)),
            vec!["bell", "silence"]
        );
        assert!(alert_types(&win("a", 0, "w", false, false, false)).is_empty());
        assert_eq!(
            alert_types(&win("a", 0, "w", true, true, true)),
            vec!["bell", "activity", "silence"]
        );
    }

    #[test]
    fn only_flagged_windows_are_reported() {
        let rows = build_rows(&snap(vec![
            win("a", 0, "quiet", false, false, false), // no alert → excluded
            win("a", 1, "loud", false, true, false),   // activity → included
        ]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].window, 1);
        assert_eq!(rows[0].alerts, vec!["activity"]);
    }

    #[test]
    fn rows_sorted_by_session_then_window() {
        let rows = build_rows(&snap(vec![
            win("z", 0, "w", true, false, false),
            win("a", 2, "w", true, false, false),
            win("a", 1, "w", true, false, false),
        ]));
        assert_eq!(
            rows.iter()
                .map(|r| (r.session.as_str(), r.window))
                .collect::<Vec<_>>(),
            vec![("a", 1), ("a", 2), ("z", 0)],
        );
    }

    #[test]
    fn text_joins_multiple_alerts_with_commas() {
        let rows = build_rows(&snap(vec![win("a", 3, "build", true, true, false)]));
        let s = render_text(&rows, false);
        assert!(s.contains("SESSION") && s.contains("ALERTS"));
        assert!(
            s.lines()
                .any(|l| l.contains("3:build") && l.contains("bell,activity"))
        );
    }

    #[test]
    fn json_carries_alerts_array() {
        let rows = build_rows(&snap(vec![win("a", 3, "build", false, false, true)]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["session"], "a");
        assert_eq!(v[0]["window"], 3);
        assert_eq!(v[0]["alerts"][0], "silence");
    }
}
