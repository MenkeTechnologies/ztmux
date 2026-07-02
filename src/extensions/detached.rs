//! `ztmux detached` — sessions with no client attached, freshest first.
//!
//! Where [`super::recent`] ranks *every* session by last activity and
//! [`super::age`] ranks them by creation time, `detached` filters to the ones
//! nobody is looking at — the sessions running in the background with no client
//! attached — and ranks those by last activity so the one you most likely want
//! to reattach sorts to the top, and the stale ones you can safely kill sink to
//! the bottom. It is the "what is running that I've walked away from" view. A
//! compact relative age is shown for the terminal; with `-o json` / `--json` the
//! raw unix timestamps are emitted. An all-attached server prints just the
//! header.

use std::io::IsTerminal;
use std::time::{SystemTime, UNIX_EPOCH};

use super::tmux_query::{Snapshot, poll};

/// One output row: a detached session with its window count and timestamps.
struct Row {
    name: String,
    windows: i64,
    activity: i64,
    created: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux detached: {e}");
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
        print!(
            "{}",
            render_text(&rows, now_unix(), std::io::stdout().is_terminal())
        );
    }
    0
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// One row per *detached* session (no client attached), most-recently-active
/// first; ties break by name for a stable order.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .sessions
        .iter()
        .filter(|s| !s.attached)
        .map(|s| Row {
            name: s.name.clone(),
            windows: s.windows,
            activity: s.activity,
            created: s.created,
        })
        .collect();
    rows.sort_by(|a, b| b.activity.cmp(&a.activity).then(a.name.cmp(&b.name)));
    rows
}

/// Format a duration in seconds as a compact relative age. Matches the
/// single-unit rendering used by [`super::recent`] and [`super::age`].
fn ago(secs: i64) -> String {
    if secs <= 0 {
        return "just now".to_string();
    }
    let d = secs / 86_400;
    let h = secs / 3_600;
    let m = secs / 60;
    if d > 0 {
        format!("{d}d")
    } else if h > 0 {
        format!("{h}h")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

fn render_text(rows: &[Row], now: i64, color: bool) -> String {
    let paint = |s: &str, code: &str| -> String {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let age = |t: i64| -> String {
        if t <= 0 {
            "-".to_string()
        } else {
            ago(now - t)
        }
    };
    let mut out = String::new();
    out.push_str(&format!(
        "{}\n",
        paint(
            &format!(
                "{:<20} {:>7} {:>8} {:>8}",
                "SESSION", "WINDOWS", "ACTIVITY", "CREATED"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<20} {:>7} {:>8} {:>8}\n",
            r.name,
            r.windows,
            age(r.activity),
            age(r.created),
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
                "activity": r.activity,
                "created": r.created,
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
    use super::super::tmux_query::Session;
    use super::*;

    fn session(name: &str, windows: i64, attached: bool, activity: i64, created: i64) -> Session {
        Session {
            name: name.into(),
            windows,
            attached,
            activity,
            created,
            ..Default::default()
        }
    }

    fn snap(sessions: Vec<Session>) -> Snapshot {
        Snapshot {
            sessions,
            ..Default::default()
        }
    }

    #[test]
    fn only_detached_sessions_are_reported() {
        let rows = build_rows(&snap(vec![
            session("live", 1, true, 9_000, 100),
            session("bg", 2, false, 5_000, 200),
        ]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "bg");
    }

    #[test]
    fn detached_sessions_sort_most_recently_active_first() {
        let rows = build_rows(&snap(vec![
            session("stale", 1, false, 1_000, 100),
            session("recent", 2, false, 8_000, 200),
        ]));
        assert_eq!(rows[0].name, "recent");
        assert_eq!(rows[1].name, "stale");
    }

    #[test]
    fn all_attached_renders_header_only() {
        let rows = build_rows(&snap(vec![session("live", 1, true, 9_000, 100)]));
        assert!(rows.is_empty());
        let s = render_text(&rows, 9_060, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("SESSION") && s.contains("ACTIVITY"));
    }

    #[test]
    fn text_renders_relative_ages_against_now() {
        let rows = build_rows(&snap(vec![session("bg", 2, false, 9_000, 100)]));
        // now = 9_060 → activity 9_000 is 60s ago → "1m".
        let s = render_text(&rows, 9_060, false);
        assert!(s.lines().any(|l| l.contains("bg") && l.contains("1m")));
    }

    #[test]
    fn json_keeps_raw_timestamps() {
        let rows = build_rows(&snap(vec![session("bg", 2, false, 9_000, 100)]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["name"], "bg");
        assert_eq!(v[0]["activity"], 9_000);
        assert_eq!(v[0]["created"], 100);
    }
}
