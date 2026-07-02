//! `ztmux history` — every pane's scrollback buffer, biggest first.
//!
//! Each pane keeps up to `history-limit` lines of scrollback; a runaway `cat` or
//! a chatty log tail can pin thousands of lines of memory in a pane you forgot
//! about. `history` ranks the panes by how many lines they are actually holding,
//! largest first, with how full each is against its own limit \(em the "which
//! pane is hoarding scrollback" view. Where [`super::size`] reports on-screen
//! geometry, `history` reports the off-screen buffer behind it. Dead panes are
//! skipped. With `-o json` / `--json` it emits the same rows as an array.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a pane and its scrollback occupancy.
struct Row {
    id: String,
    location: String,
    command: String,
    lines: i64,
    limit: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux history: {e}");
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

fn location(p: &Pane) -> String {
    format!("{}:{}.{}", p.session, p.window, p.index)
}

/// Percent of the scrollback limit currently used (`None` when the limit is 0,
/// i.e. scrollback disabled, to avoid dividing by zero).
fn full_pct(lines: i64, limit: i64) -> Option<i64> {
    (limit > 0).then(|| lines * 100 / limit)
}

/// One row per live pane, ordered by scrollback lines held, largest first; ties
/// break by location for a stable order.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead)
        .map(|p| Row {
            id: p.id.clone(),
            location: location(p),
            command: p.command.clone(),
            lines: p.history_size,
            limit: p.history_limit,
        })
        .collect();
    rows.sort_by(|a, b| b.lines.cmp(&a.lines).then(a.location.cmp(&b.location)));
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
                "{:<8} {:<16} {:<12} {:>8} {:>8} {:>6}",
                "PANE", "LOCATION", "COMMAND", "LINES", "LIMIT", "FULL%"
            ),
            "1"
        )
    ));
    for r in rows {
        let full = full_pct(r.lines, r.limit).map_or_else(|| "-".to_string(), |p| format!("{p}%"));
        out.push_str(&format!(
            "{:<8} {:<16} {:<12} {:>8} {:>8} {:>6}\n",
            r.id, r.location, r.command, r.lines, r.limit, full,
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "location": r.location,
                "command": r.command,
                "lines": r.lines,
                "limit": r.limit,
                "full_pct": full_pct(r.lines, r.limit),
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

    fn pane(id: &str, sess: &str, win: i64, idx: i64, size: i64, limit: i64) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: "zsh".into(),
            history_size: size,
            history_limit: limit,
            ..Default::default()
        }
    }

    fn snap(panes: Vec<Pane>) -> Snapshot {
        Snapshot {
            panes,
            ..Default::default()
        }
    }

    #[test]
    fn biggest_scrollback_sorts_first() {
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, 50, 2000),
            pane("%2", "a", 1, 0, 9000, 10000),
            pane("%3", "a", 2, 0, 300, 2000),
        ]));
        assert_eq!(rows[0].id, "%2");
        assert_eq!(rows[0].lines, 9000);
        assert_eq!(rows[1].id, "%3");
        assert_eq!(rows[2].id, "%1");
    }

    #[test]
    fn full_pct_guards_zero_limit() {
        assert_eq!(full_pct(500, 2000), Some(25));
        assert_eq!(full_pct(10000, 10000), Some(100));
        assert_eq!(full_pct(5, 0), None);
    }

    #[test]
    fn dead_panes_are_skipped() {
        let mut dead = pane("%2", "a", 1, 0, 9999, 10000);
        dead.dead = true;
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, 10, 10000), dead]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%1");
    }

    #[test]
    fn text_shows_full_percent_and_dash_for_zero_limit() {
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, 500, 2000),
            pane("%2", "a", 1, 0, 5, 0),
        ]));
        let s = render_text(&rows, false);
        assert!(s.contains("LINES") && s.contains("FULL%"));
        assert!(s.lines().any(|l| l.contains("%1") && l.contains("25%")));
        let z = s.lines().find(|l| l.contains("%2")).unwrap();
        assert!(z.trim_end().ends_with(" -"));
    }

    #[test]
    fn json_carries_lines_limit_and_pct() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, 500, 2000)]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["lines"], 500);
        assert_eq!(v[0]["limit"], 2000);
        assert_eq!(v[0]["full_pct"], 25);
    }
}
