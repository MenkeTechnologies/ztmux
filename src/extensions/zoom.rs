//! `ztmux zoom` — every window that currently has a zoomed pane.
//!
//! A zoomed window shows one pane full-screen and hides the rest until you
//! un-zoom — easy to leave set in one of many windows and then wonder where the
//! other panes went. `zoom` lists exactly the windows that are zoomed right now
//! and the pane each is zoomed to (the active pane), so a stray zoom is a `grep`
//! away. Windows in their normal split layout are omitted. Sorted by session then
//! window. With `-o json` / `--json` it emits the same rows as an array.

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, poll};

/// One output row: a zoomed window and the pane it is zoomed to.
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
        eprintln!("ztmux zoom: {e}");
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

/// One row per zoomed window, paired with its active pane (the pane a zoom shows
/// full-screen). A zoomed window with no resolvable active pane is skipped.
/// Ordered by session then window index.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .windows
        .iter()
        .filter(|w| w.zoomed)
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

    fn win(session: &str, index: i64, name: &str, zoomed: bool) -> Window {
        Window {
            session: session.into(),
            index,
            name: name.into(),
            zoomed,
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
    fn only_zoomed_windows_with_their_active_pane() {
        let s = snap(
            vec![win("a", 0, "edit", false), win("a", 1, "run", true)],
            vec![
                pane("a", 0, 0, "%1", true, "vim"), // active pane of a non-zoomed window
                pane("a", 1, 0, "%3", false, "zsh"),
                pane("a", 1, 1, "%4", true, "top"), // the zoomed (active) pane
            ],
        );
        let rows = build_rows(&s);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].window, 1);
        assert_eq!(rows[0].pane, "%4");
        assert_eq!(rows[0].command, "top");
    }

    #[test]
    fn no_zoomed_windows_yields_no_rows() {
        let s = snap(
            vec![win("a", 0, "w", false)],
            vec![pane("a", 0, 0, "%1", true, "zsh")],
        );
        assert!(build_rows(&s).is_empty());
    }

    #[test]
    fn rows_sorted_by_session_then_window() {
        let s = snap(
            vec![
                win("z", 0, "w", true),
                win("a", 3, "w", true),
                win("a", 1, "w", true),
            ],
            vec![
                pane("z", 0, 0, "%9", true, "zsh"),
                pane("a", 3, 0, "%3", true, "zsh"),
                pane("a", 1, 0, "%1", true, "zsh"),
            ],
        );
        let rows = build_rows(&s);
        assert_eq!(
            rows.iter()
                .map(|r| (r.session.as_str(), r.window))
                .collect::<Vec<_>>(),
            vec![("a", 1), ("a", 3), ("z", 0)],
        );
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
    fn json_carries_zoom_fields() {
        let s = snap(
            vec![win("a", 2, "build", true)],
            vec![pane("a", 2, 0, "%5", true, "make")],
        );
        let v: serde_json::Value = serde_json::from_str(&render_json(&build_rows(&s))).unwrap();
        assert_eq!(v[0]["session"], "a");
        assert_eq!(v[0]["window"], 2);
        assert_eq!(v[0]["pane"], "%5");
        assert_eq!(v[0]["command"], "make");
    }
}
