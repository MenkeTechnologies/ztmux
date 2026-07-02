//! `ztmux active` — the focused window and pane of every session.
//!
//! Each session has exactly one active window, and that window one active pane:
//! the spot your cursor lands when you attach. `active` prints that focus point
//! for every session — session, its active window, the active pane inside it,
//! and what that pane is running/where. Where [`super::tree`] prints the whole
//! structure and [`super::recent`] ranks by activity, `active` is the one-line
//! "where is focus, everywhere" jump map. Sorted by session. With `-o json` /
//! `--json` it emits the same rows as a machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, poll};

/// One output row: a session and its current focus (active window + pane).
struct Row {
    session: String,
    window: i64,
    window_name: String,
    pane: String,
    command: String,
    path: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux active: {e}");
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

/// For every active window, pair it with the active pane inside it. A window
/// with no resolvable active pane is skipped (tmux keeps exactly one, so this
/// only drops genuine data gaps). Ordered by session for a stable list.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .windows
        .iter()
        .filter(|w| w.active)
        .filter_map(|w| {
            let pane = snap
                .panes
                .iter()
                .find(|p| p.active && p.session == w.session && p.window == w.index)?;
            Some(Row {
                session: w.session.clone(),
                window: w.index,
                window_name: w.name.clone(),
                pane: pane.id.clone(),
                command: pane.command.clone(),
                path: pane.path.clone(),
            })
        })
        .collect();
    rows.sort_by(|a, b| a.session.cmp(&b.session));
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
                "{:<20} {:<16} {:<8} {:<12} {}",
                "SESSION", "WINDOW", "PANE", "COMMAND", "PATH"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<20} {:<16} {:<8} {:<12} {}\n",
            r.session,
            format!("{}:{}", r.window, r.window_name),
            r.pane,
            r.command,
            r.path,
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
                "pane": r.pane,
                "command": r.command,
                "path": r.path,
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
    use super::super::tmux_query::{Pane, Window};
    use super::*;

    fn win(session: &str, index: i64, name: &str, active: bool) -> Window {
        Window {
            session: session.into(),
            index,
            name: name.into(),
            active,
            ..Default::default()
        }
    }

    fn pane(session: &str, window: i64, index: i64, id: &str, active: bool, cmd: &str) -> Pane {
        Pane {
            session: session.into(),
            window,
            index,
            id: id.into(),
            active,
            command: cmd.into(),
            path: "/w".into(),
            ..Default::default()
        }
    }

    fn snap(windows: Vec<Window>, panes: Vec<Pane>) -> Snapshot {
        Snapshot {
            windows,
            panes,
            ..Default::default()
        }
    }

    #[test]
    fn picks_active_window_and_its_active_pane() {
        let s = snap(
            vec![win("a", 0, "edit", false), win("a", 1, "run", true)],
            vec![
                pane("a", 1, 0, "%3", false, "zsh"),
                pane("a", 1, 1, "%4", true, "nvim"),
                pane("a", 0, 0, "%1", true, "top"), // active pane of the INACTIVE window
            ],
        );
        let rows = build_rows(&s);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].window, 1);
        assert_eq!(rows[0].window_name, "run");
        assert_eq!(rows[0].pane, "%4");
        assert_eq!(rows[0].command, "nvim");
    }

    #[test]
    fn one_row_per_session_sorted_by_session() {
        let s = snap(
            vec![win("z", 0, "w", true), win("a", 0, "w", true)],
            vec![
                pane("z", 0, 0, "%9", true, "zsh"),
                pane("a", 0, 0, "%1", true, "zsh"),
            ],
        );
        let rows = build_rows(&s);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].session, "a");
        assert_eq!(rows[1].session, "z");
    }

    #[test]
    fn active_window_without_a_matching_active_pane_is_skipped() {
        // Active window a:1 but no active pane recorded for it.
        let s = snap(
            vec![win("a", 1, "run", true)],
            vec![pane("a", 1, 0, "%3", false, "zsh")],
        );
        assert!(build_rows(&s).is_empty());
    }

    #[test]
    fn text_has_header_and_window_index_name() {
        let s = snap(
            vec![win("a", 2, "build", true)],
            vec![pane("a", 2, 0, "%5", true, "make")],
        );
        let out = render_text(&build_rows(&s), false);
        assert!(out.contains("SESSION") && out.contains("PANE"));
        assert!(
            out.lines()
                .any(|l| l.contains("2:build") && l.contains("%5"))
        );
    }

    #[test]
    fn json_carries_focus_fields() {
        let s = snap(
            vec![win("a", 2, "build", true)],
            vec![pane("a", 2, 0, "%5", true, "make")],
        );
        let v: serde_json::Value = serde_json::from_str(&render_json(&build_rows(&s))).unwrap();
        assert_eq!(v[0]["session"], "a");
        assert_eq!(v[0]["window"], 2);
        assert_eq!(v[0]["window_name"], "build");
        assert_eq!(v[0]["pane"], "%5");
        assert_eq!(v[0]["command"], "make");
    }
}
