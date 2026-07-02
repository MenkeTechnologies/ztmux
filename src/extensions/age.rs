//! `ztmux age` — sessions ranked by how long they have existed, oldest first.
//!
//! The mirror image of [`super::recent`]: where `recent` ranks sessions by their
//! last *activity* (freshest work on top), `age` ranks them by their *creation*
//! timestamp with the oldest on top — the long-lived sessions that have been up
//! the longest, the ones you never tear down. It is the "what has been running
//! since forever" view, the pipeable primitive behind spotting the stale session
//! you forgot to close. A compact relative age is shown for the terminal;
//! sessions whose creation time is unknown (`0`) sort last and render as `-`.
//! With `-o json` / `--json` the raw unix creation timestamp is emitted.

use std::io::IsTerminal;
use std::time::{SystemTime, UNIX_EPOCH};

use super::tmux_query::{Snapshot, poll};

/// One output row: a session with its window count and creation time.
struct Row {
    name: String,
    windows: i64,
    attached: bool,
    created: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux age: {e}");
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

/// One row per session, oldest creation time first. A creation time of `0`
/// (unknown) is not "the beginning of the epoch" — those sessions sort last so
/// they never masquerade as the oldest. Equal creation times break by name for
/// a stable order.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .sessions
        .iter()
        .map(|s| Row {
            name: s.name.clone(),
            windows: s.windows,
            attached: s.attached,
            created: s.created,
        })
        .collect();
    rows.sort_by(|a, b| {
        // Unknown (0) creation sorts to the very end regardless of order.
        let key = |c: i64| if c <= 0 { i64::MAX } else { c };
        key(a.created)
            .cmp(&key(b.created))
            .then(a.name.cmp(&b.name))
    });
    rows
}

/// Format a duration in seconds as a compact relative age (e.g. `3h`, `2d`).
/// Matches the single-unit rendering used by [`super::recent`].
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
    let mut out = String::new();
    out.push_str(&format!(
        "{}\n",
        paint(
            &format!(
                "{:<20} {:>7} {:>8} {:>8}",
                "SESSION", "WINDOWS", "ACTIVE", "AGE"
            ),
            "1"
        )
    ));
    for r in rows {
        let age = if r.created <= 0 {
            "-".to_string()
        } else {
            ago(now - r.created)
        };
        out.push_str(&format!(
            "{:<20} {:>7} {:>8} {:>8}\n",
            r.name,
            r.windows,
            if r.attached { "yes" } else { "no" },
            age,
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
                "attached": r.attached,
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

    fn session(name: &str, windows: i64, attached: bool, created: i64) -> Session {
        Session {
            name: name.into(),
            windows,
            attached,
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
    fn oldest_creation_sorts_first() {
        let rows = build_rows(&snap(vec![
            session("new", 1, false, 9_000),
            session("old", 3, true, 500),
            session("mid", 2, false, 4_000),
        ]));
        assert_eq!(rows[0].name, "old");
        assert_eq!(rows[1].name, "mid");
        assert_eq!(rows[2].name, "new");
    }

    #[test]
    fn unknown_creation_sorts_last_not_first() {
        let rows = build_rows(&snap(vec![
            session("unknown", 1, false, 0),
            session("old", 3, true, 500),
        ]));
        assert_eq!(rows[0].name, "old");
        assert_eq!(rows[1].name, "unknown");
    }

    #[test]
    fn equal_creation_breaks_ties_by_name() {
        let rows = build_rows(&snap(vec![
            session("bbb", 1, false, 1_000),
            session("aaa", 1, false, 1_000),
        ]));
        assert_eq!(rows[0].name, "aaa");
        assert_eq!(rows[1].name, "bbb");
    }

    #[test]
    fn ago_picks_the_largest_single_unit() {
        assert_eq!(ago(45), "45s");
        assert_eq!(ago(120), "2m");
        assert_eq!(ago(7_200), "2h");
        assert_eq!(ago(172_800), "2d");
    }

    #[test]
    fn text_renders_age_against_now_and_dash_for_unknown() {
        let rows = build_rows(&snap(vec![
            session("old", 3, true, 500),
            session("unknown", 1, false, 0),
        ]));
        // now = 86_900 → old created 500 is ~86_400s ago → "1d".
        let s = render_text(&rows, 86_900, false);
        assert!(s.contains("SESSION") && s.contains("AGE"));
        let old_line = s.lines().find(|l| l.contains("old")).unwrap();
        assert!(old_line.contains("1d"));
        let unk_line = s.lines().find(|l| l.contains("unknown")).unwrap();
        assert!(unk_line.contains(" -"));
    }

    #[test]
    fn json_keeps_raw_creation_timestamp() {
        let rows = build_rows(&snap(vec![session("old", 3, true, 500)]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["name"], "old");
        assert_eq!(v[0]["created"], 500);
        assert_eq!(v[0]["attached"], true);
    }
}
