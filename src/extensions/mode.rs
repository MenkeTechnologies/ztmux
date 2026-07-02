//! `ztmux mode` — every pane currently frozen in a mode.
//!
//! When a pane enters copy-mode, view-mode, or the tree/buffer/client choosers,
//! it stops showing live output until you exit — easy to forget in one of many
//! panes. `mode` lists exactly the panes that are in a mode right now, and which
//! one, so a pane left paused in copy-mode is a `grep` away. Panes showing live
//! output (the normal case) are omitted. Sorted by location. With `-o json` /
//! `--json` it emits the same rows as a machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a pane and the mode it is currently in.
struct Row {
    id: String,
    location: String,
    command: String,
    mode: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux mode: {e}");
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

/// One row per live pane that is currently in a mode, ordered by location. Panes
/// showing live output (`in_mode == false`) are excluded.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead && p.in_mode)
        .map(|p| Row {
            id: p.id.clone(),
            location: location(p),
            command: p.command.clone(),
            // Empty mode string (in a mode the query didn't name) shows as "-".
            mode: if p.mode.is_empty() {
                "-".to_string()
            } else {
                p.mode.clone()
            },
        })
        .collect();
    rows.sort_by(|a, b| a.location.cmp(&b.location));
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
                "{:<8} {:<16} {:<12} {}",
                "PANE", "LOCATION", "COMMAND", "MODE"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<12} {}\n",
            r.id, r.location, r.command, r.mode,
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
                "mode": r.mode,
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

    fn pane(id: &str, sess: &str, win: i64, idx: i64, in_mode: bool, mode: &str) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: "zsh".into(),
            in_mode,
            mode: mode.into(),
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
    fn only_panes_in_a_mode_are_listed() {
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, false, ""),         // live → excluded
            pane("%2", "a", 1, 0, true, "copy-mode"), // in mode → included
        ]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%2");
        assert_eq!(rows[0].mode, "copy-mode");
    }

    #[test]
    fn dead_panes_are_skipped_even_if_flagged_in_mode() {
        let mut dead = pane("%2", "a", 1, 0, true, "copy-mode");
        dead.dead = true;
        assert!(build_rows(&snap(vec![dead])).is_empty());
    }

    #[test]
    fn rows_sorted_by_location() {
        let rows = build_rows(&snap(vec![
            pane("%9", "z", 5, 0, true, "view-mode"),
            pane("%1", "a", 0, 0, true, "copy-mode"),
        ]));
        assert_eq!(rows[0].location, "a:0.0");
        assert_eq!(rows[1].location, "z:5.0");
    }

    #[test]
    fn empty_mode_name_renders_as_dash() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, true, "")]));
        assert_eq!(rows[0].mode, "-");
    }

    #[test]
    fn json_carries_mode() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, true, "copy-mode")]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%1");
        assert_eq!(v[0]["mode"], "copy-mode");
    }
}
