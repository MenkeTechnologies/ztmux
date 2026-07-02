//! `ztmux focus` — the active pane of every window: where the cursor lands.
//!
//! Each window has exactly one active pane — the pane your cursor lands in when
//! you select that window. Where [`super::active`] reports focus once *per
//! session* (the session's active window and its active pane), `focus` reports
//! it once *per window*, so you see the landing pane for every window, not just
//! the frontmost one — the "if I switch to window N, where do I end up" map. Each
//! row shows the window, the active pane, and what it is running/where. Sorted by
//! window location. With `-o json` / `--json` it emits the same rows as a
//! machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, Window, poll};

/// One output row: a window and the active pane inside it.
struct Row {
    location: String, // session:index
    name: String,
    pane: String, // active pane id (%N)
    command: String,
    path: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux focus: {e}");
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

fn location(w: &Window) -> String {
    format!("{}:{}", w.session, w.index)
}

/// One row per window, carrying that window's active pane (matched by session
/// and window index). A window with no active pane found (e.g. its active pane
/// is dead) still gets a row, with the pane columns left blank. Ordered by
/// window location for a stable table.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .windows
        .iter()
        .map(|w| {
            let active = snap
                .panes
                .iter()
                .find(|p| p.session == w.session && p.window == w.index && p.active);
            Row {
                location: location(w),
                name: w.name.clone(),
                pane: active.map(|p| p.id.clone()).unwrap_or_default(),
                command: active.map(|p| p.command.clone()).unwrap_or_default(),
                path: active.map(|p| p.path.clone()).unwrap_or_default(),
            }
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
                "{:<12} {:<16} {:<8} {:<12} {}",
                "WINDOW", "NAME", "PANE", "COMMAND", "PATH"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<12} {:<16} {:<8} {:<12} {}\n",
            r.location, r.name, r.pane, r.command, r.path,
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "window": r.location,
                "name": r.name,
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
    use super::super::tmux_query::Pane;
    use super::*;

    fn window(sess: &str, idx: i64, name: &str) -> Window {
        Window {
            session: sess.into(),
            index: idx,
            name: name.into(),
            ..Default::default()
        }
    }

    fn pane(id: &str, sess: &str, win: i64, idx: i64, cmd: &str, active: bool) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: cmd.into(),
            path: "/src".into(),
            active,
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
    fn each_window_gets_its_active_pane() {
        let rows = build_rows(&snap(
            vec![window("a", 0, "edit"), window("a", 1, "run")],
            vec![
                pane("%0", "a", 0, 0, "zsh", false),
                pane("%1", "a", 0, 1, "vim", true),
                pane("%2", "a", 1, 0, "cargo", true),
            ],
        ));
        assert_eq!(rows[0].location, "a:0");
        assert_eq!(rows[0].pane, "%1");
        assert_eq!(rows[0].command, "vim");
        assert_eq!(rows[1].location, "a:1");
        assert_eq!(rows[1].pane, "%2");
    }

    #[test]
    fn window_without_active_pane_gets_blank_columns() {
        let rows = build_rows(&snap(
            vec![window("a", 0, "edit")],
            vec![pane("%0", "a", 0, 0, "zsh", false)],
        ));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pane, "");
        assert_eq!(rows[0].command, "");
    }

    #[test]
    fn rows_sorted_by_window_location() {
        let rows = build_rows(&snap(
            vec![window("z", 9, "b"), window("a", 0, "a")],
            vec![],
        ));
        assert_eq!(rows[0].location, "a:0");
        assert_eq!(rows[1].location, "z:9");
    }

    #[test]
    fn text_renders_window_pane_and_command() {
        let rows = build_rows(&snap(
            vec![window("a", 0, "edit")],
            vec![pane("%1", "a", 0, 1, "vim", true)],
        ));
        let s = render_text(&rows, false);
        assert!(s.contains("WINDOW") && s.contains("PANE") && s.contains("COMMAND"));
        assert!(
            s.lines()
                .any(|l| l.contains("a:0") && l.contains("%1") && l.contains("vim"))
        );
    }

    #[test]
    fn json_carries_window_active_pane_and_command() {
        let rows = build_rows(&snap(
            vec![window("a", 0, "edit")],
            vec![pane("%1", "a", 0, 1, "vim", true)],
        ));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["window"], "a:0");
        assert_eq!(v[0]["pane"], "%1");
        assert_eq!(v[0]["command"], "vim");
        assert_eq!(v[0]["path"], "/src");
    }
}
