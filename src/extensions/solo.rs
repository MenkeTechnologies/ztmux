//! `ztmux solo` — windows that hold a single, unsplit pane.
//!
//! The complement of [`super::density`] (which ranks windows by how *many* panes
//! they hold): `solo` lists the windows with exactly one pane — the unsplit
//! windows, the ones you could split further or fold into another window. Each
//! row also shows what that lone pane is running, so it doubles as the "which
//! windows are a single bare shell" view. With `-o json` / `--json` it emits the
//! same rows as a machine-readable array; a server with no single-pane windows
//! prints just the header.

use std::io::IsTerminal;

use super::tmux_query::{Snapshot, Window, poll};

/// One output row: a single-pane window and the command in that pane.
struct Row {
    location: String, // session:index
    name: String,
    command: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux solo: {e}");
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

/// One row per window whose pane count is exactly one, ordered by location. The
/// lone pane's command is looked up from the pane list (the pane sharing the
/// window's session and index); if none is found it is left blank.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .windows
        .iter()
        .filter(|w| w.panes == 1)
        .map(|w| {
            let command = snap
                .panes
                .iter()
                .find(|p| p.session == w.session && p.window == w.index && !p.dead)
                .map(|p| p.command.clone())
                .unwrap_or_default();
            Row {
                location: location(w),
                name: w.name.clone(),
                command,
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
            &format!("{:<12} {:<16} {}", "WINDOW", "NAME", "COMMAND"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<12} {:<16} {}\n",
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
                "command": r.command,
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

    fn window(sess: &str, idx: i64, name: &str, panes: i64) -> Window {
        Window {
            session: sess.into(),
            index: idx,
            name: name.into(),
            panes,
            ..Default::default()
        }
    }

    fn pane(sess: &str, win: i64, idx: i64, cmd: &str) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            command: cmd.into(),
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
    fn only_single_pane_windows_are_reported() {
        let rows = build_rows(&snap(
            vec![
                window("a", 0, "edit", 1),
                window("a", 1, "run", 3),
                window("a", 2, "logs", 1),
            ],
            vec![],
        ));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].location, "a:0");
        assert_eq!(rows[1].location, "a:2");
    }

    #[test]
    fn lone_pane_command_is_joined_in() {
        let rows = build_rows(&snap(
            vec![window("a", 0, "edit", 1)],
            vec![pane("a", 0, 0, "vim")],
        ));
        assert_eq!(rows[0].command, "vim");
    }

    #[test]
    fn rows_sorted_by_location() {
        let rows = build_rows(&snap(
            vec![window("z", 9, "b", 1), window("a", 0, "a", 1)],
            vec![],
        ));
        assert_eq!(rows[0].location, "a:0");
        assert_eq!(rows[1].location, "z:9");
    }

    #[test]
    fn no_solo_windows_renders_header_only() {
        let rows = build_rows(&snap(vec![window("a", 0, "edit", 4)], vec![]));
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("WINDOW") && s.contains("COMMAND"));
    }

    #[test]
    fn json_carries_window_name_and_command() {
        let rows = build_rows(&snap(
            vec![window("a", 0, "edit", 1)],
            vec![pane("a", 0, 0, "vim")],
        ));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["window"], "a:0");
        assert_eq!(v[0]["name"], "edit");
        assert_eq!(v[0]["command"], "vim");
    }
}
