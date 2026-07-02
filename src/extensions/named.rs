//! `ztmux named` — windows whose name no longer tracks the running command.
//!
//! With tmux's `automatic-rename` on, a window's name mirrors its active pane's
//! command (`vim`, `zsh`, …). A window whose name *differs* from that command is
//! one you have deliberately named — with `rename-window`, or by turning
//! automatic-rename off so a chosen name sticks. `named` lists exactly those
//! windows: the ones carrying a meaningful label rather than an echo of whatever
//! is running. It is the "which windows did I bother to name, and what did I call
//! them" view — the navigation map for `select-window -t <name>`. Each row shows
//! the label and the command it diverges from. Windows with an empty name, or
//! whose name still equals the active command, are omitted. Sorted by location.
//! With `-o json` / `--json` it emits the same rows as a machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, Window, poll};

/// One output row: a deliberately-named window and its active command.
struct Row {
    location: String, // session:index
    name: String,
    command: String, // active pane's command the name diverges from
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux named: {e}");
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

/// One row per window whose (non-empty) name differs from its active pane's
/// command — the windows carrying a deliberate label. A window with no active
/// pane found contributes an empty command, so any non-empty name still counts
/// as deliberate. Ordered by window location.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .windows
        .iter()
        .filter_map(|w| {
            if w.name.is_empty() {
                return None;
            }
            let command = snap
                .panes
                .iter()
                .find(|p| p.session == w.session && p.window == w.index && p.active)
                .map(|p| p.command.clone())
                .unwrap_or_default();
            if w.name == command {
                return None;
            }
            Some(Row {
                location: location(w),
                name: w.name.clone(),
                command,
            })
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
            &format!("{:<12} {:<20} {}", "WINDOW", "NAME", "RUNNING"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<12} {:<20} {}\n",
            r.location, r.name, r.command
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
                "running": r.command,
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

    fn pane(sess: &str, win: i64, idx: i64, cmd: &str, active: bool) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            command: cmd.into(),
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
    fn window_named_same_as_command_is_omitted() {
        let rows = build_rows(&snap(
            vec![window("a", 0, "vim")],
            vec![pane("a", 0, 0, "vim", true)],
        ));
        assert!(rows.is_empty());
    }

    #[test]
    fn window_with_deliberate_name_is_reported() {
        let rows = build_rows(&snap(
            vec![window("a", 0, "deploy")],
            vec![pane("a", 0, 0, "zsh", true)],
        ));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "deploy");
        assert_eq!(rows[0].command, "zsh");
    }

    #[test]
    fn empty_named_window_is_omitted() {
        let rows = build_rows(&snap(
            vec![window("a", 0, "")],
            vec![pane("a", 0, 0, "zsh", true)],
        ));
        assert!(rows.is_empty());
    }

    #[test]
    fn name_without_active_pane_counts_as_deliberate() {
        let rows = build_rows(&snap(vec![window("a", 0, "deploy")], vec![]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "deploy");
        assert_eq!(rows[0].command, "");
    }

    #[test]
    fn rows_sorted_by_location() {
        let rows = build_rows(&snap(
            vec![window("z", 9, "b"), window("a", 0, "a")],
            vec![],
        ));
        assert_eq!(rows[0].location, "a:0");
        assert_eq!(rows[1].location, "z:9");
    }

    #[test]
    fn json_carries_window_name_and_running() {
        let rows = build_rows(&snap(
            vec![window("a", 0, "deploy")],
            vec![pane("a", 0, 0, "zsh", true)],
        ));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["window"], "a:0");
        assert_eq!(v[0]["name"], "deploy");
        assert_eq!(v[0]["running"], "zsh");
    }
}
