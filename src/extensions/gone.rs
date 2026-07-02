//! `ztmux gone` — panes whose working directory no longer exists on disk.
//!
//! A long-lived pane keeps the working directory it was started in even after
//! that directory is deleted, renamed, or unmounted out from under it — new
//! commands then fail with "no such file or directory" for no obvious reason.
//! `gone` finds those panes: it checks each live pane's recorded working
//! directory against the filesystem and reports the ones that are no longer
//! there, so you can `cd` them somewhere real. Where [`super::cwd`] groups panes
//! by directory and [`super::dead`] lists dead *processes*, `gone` flags a live
//! pane in a dead *place*. It is filesystem-only and checks each unique directory
//! once. A server whose panes are all in real directories prints just the header.
//! With `-o json` / `--json` it emits the same rows as a machine-readable array.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::Path;

use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a live pane sitting in a directory that is gone.
struct Row {
    id: String,
    location: String,
    command: String,
    path: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux gone: {e}");
        return 1;
    }
    let rows = build_rows(&snap, |p| Path::new(p).exists());
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

/// One row per live pane whose (non-empty) working directory does not exist,
/// ordered by location. `exists` is injected so the check is testable without
/// touching the filesystem; each unique path is checked at most once.
fn build_rows<F: Fn(&str) -> bool>(snap: &Snapshot, exists: F) -> Vec<Row> {
    let mut cache: HashMap<&str, bool> = HashMap::new();
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead && !p.path.is_empty())
        .filter(|p| {
            let there = *cache
                .entry(p.path.as_str())
                .or_insert_with(|| exists(&p.path));
            !there
        })
        .map(|p| Row {
            id: p.id.clone(),
            location: location(p),
            command: p.command.clone(),
            path: p.path.clone(),
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
                "PANE", "LOCATION", "COMMAND", "PATH"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<12} {}\n",
            r.id, r.location, r.command, r.path,
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
    use super::*;

    fn pane(id: &str, idx: i64, path: &str) -> Pane {
        Pane {
            id: id.into(),
            session: "a".into(),
            window: 0,
            index: idx,
            path: path.into(),
            command: "zsh".into(),
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
    fn only_missing_directories_are_reported() {
        let sn = snap(vec![
            pane("%1", 0, "/real"),
            pane("%2", 1, "/deleted"),
            pane("%3", 2, "/also-real"),
        ]);
        let rows = build_rows(&sn, |p| p != "/deleted");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%2");
        assert_eq!(rows[0].path, "/deleted");
    }

    #[test]
    fn dead_and_pathless_panes_are_skipped() {
        let mut dead = pane("%2", 1, "/deleted");
        dead.dead = true;
        let sn = snap(vec![pane("%1", 0, ""), dead, pane("%3", 2, "/deleted")]);
        // Only %3 is a live, pathful pane in a missing dir.
        let rows = build_rows(&sn, |_| false);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%3");
    }

    #[test]
    fn all_real_renders_header_only() {
        let sn = snap(vec![pane("%1", 0, "/real")]);
        let rows = build_rows(&sn, |_| true);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("PANE") && s.contains("PATH"));
    }

    #[test]
    fn rows_sorted_by_location() {
        let sn = snap(vec![pane("%9", 9, "/x"), pane("%1", 0, "/x")]);
        let rows = build_rows(&sn, |_| false);
        assert_eq!(rows[0].location, "a:0.0");
        assert_eq!(rows[1].location, "a:0.9");
    }

    #[test]
    fn json_carries_path() {
        let sn = snap(vec![pane("%1", 0, "/deleted")]);
        let rows = build_rows(&sn, |_| false);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%1");
        assert_eq!(v[0]["path"], "/deleted");
    }
}
