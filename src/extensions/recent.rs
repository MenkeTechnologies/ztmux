//! `ztmux recent` — sessions ranked by last activity, most-recent first.
//!
//! A one-shot client subcommand over the `list-* -o json` query layer. Where
//! [`super::stats`] rolls the server up into aggregate totals and [`super::tree`]
//! prints structure, this ranks sessions by their `session_activity` timestamp
//! so the freshest work sorts to the top — the pipeable primitive behind
//! "jump back to what I was last doing". Columns show a compact relative age for
//! both activity and creation. With `-o json` / `--json` it emits the same rows
//! as a machine-readable array (activity/created kept as raw unix seconds).

use std::io::IsTerminal;
use std::time::{SystemTime, UNIX_EPOCH};

use super::tmux_query::{Snapshot, poll};

/// One output row: a session with its window count and activity/creation times.
struct Row {
    name: String,
    windows: i64,
    attached: bool,
    activity: i64,
    created: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux recent: {e}");
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

fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .sessions
        .iter()
        .map(|s| Row {
            name: s.name.clone(),
            windows: s.windows,
            attached: s.attached,
            activity: s.activity,
            created: s.created,
        })
        .collect();
    // Most-recently-active first; ties broken by name for a stable order.
    rows.sort_by(|a, b| b.activity.cmp(&a.activity).then(a.name.cmp(&b.name)));
    rows
}

/// Format a duration in seconds as a compact relative age (e.g. `3h`, `2d`,
/// `just now`). Mirrors the buckets used by `stats`'s `human_age` but renders a
/// single unit for a narrow column.
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
                "{:<20} {:>7} {:>8} {:>8} {:>8}",
                "SESSION", "WINDOWS", "ACTIVE", "ACTIVITY", "CREATED"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<20} {:>7} {:>8} {:>8} {:>8}\n",
            r.name,
            r.windows,
            if r.attached { "yes" } else { "no" },
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
                "attached": r.attached,
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

    fn snap() -> Snapshot {
        Snapshot {
            sessions: vec![
                Session {
                    name: "old".into(),
                    windows: 2,
                    attached: false,
                    activity: 1_000,
                    created: 500,
                    ..Default::default()
                },
                Session {
                    name: "fresh".into(),
                    windows: 5,
                    attached: true,
                    activity: 9_000,
                    created: 8_000,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn rows_sorted_most_recent_activity_first() {
        let rows = build_rows(&snap());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "fresh");
        assert_eq!(rows[1].name, "old");
    }

    #[test]
    fn equal_activity_breaks_ties_by_name() {
        let mut sn = snap();
        sn.sessions[0].activity = 9_000;
        sn.sessions[0].name = "aaa".into();
        let rows = build_rows(&sn);
        assert_eq!(rows[0].name, "aaa");
    }

    #[test]
    fn ago_picks_the_largest_single_unit() {
        assert_eq!(ago(0), "just now");
        assert_eq!(ago(45), "45s");
        assert_eq!(ago(120), "2m");
        assert_eq!(ago(7_200), "2h");
        assert_eq!(ago(172_800), "2d");
    }

    #[test]
    fn text_renders_relative_ages_against_now() {
        let rows = build_rows(&snap());
        // now = 9_060 → fresh activity 9_000 is 60s ago → "1m".
        let s = render_text(&rows, 9_060, false);
        assert!(s.contains("SESSION") && s.contains("ACTIVITY"));
        assert!(s.contains("fresh") && s.contains("1m"));
    }

    #[test]
    fn json_keeps_raw_unix_timestamps() {
        let rows = build_rows(&snap());
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["name"], "fresh");
        assert_eq!(v[0]["activity"], 9_000);
        assert_eq!(v[0]["attached"], true);
    }

    // A zero timestamp (never active / unknown creation) renders as "-", not a
    // huge age computed against the epoch.
    #[test]
    fn zero_timestamp_renders_as_dash() {
        let mut sn = snap();
        sn.sessions[1].activity = 0;
        let rows = build_rows(&sn);
        let s = render_text(&rows, 9_060, false);
        let line = s.lines().find(|l| l.contains("fresh")).unwrap();
        assert!(line.contains(" -"));
    }
}
