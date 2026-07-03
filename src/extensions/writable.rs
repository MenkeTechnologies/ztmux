//! `ztmux writable` — panes whose working directory you cannot write to.
//!
//! A pane can sit in a directory that exists but is read-only to you — a
//! root-owned tree, a read-only mount, someone else's checkout — and the first
//! write (`git commit`, `touch`, a build output) fails for a reason that is not
//! obvious from the prompt. `writable` finds those panes: it checks each live
//! pane's working directory for write access with `access(2)` and reports the
//! ones that exist but deny it. Where [`super::gone`] flags a directory that is
//! *missing*, `writable` flags one that is present but *read-only*. It is
//! filesystem-only and checks each unique directory once. A server whose panes
//! can all write prints just the header. With `-o json` / `--json` it emits the
//! same rows as a machine-readable array.

use std::collections::HashMap;
use std::ffi::CString;
use std::io::IsTerminal;
use std::path::Path;

use super::tmux_query::{Pane, Snapshot, poll};

/// The write-access state of a directory.
#[derive(Clone, Copy, PartialEq)]
enum Access {
    Writable,
    ReadOnly,
    Missing,
}

/// One output row: a live pane in an existing but read-only directory.
struct Row {
    id: String,
    location: String,
    command: String,
    path: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux writable: {e}");
        return 1;
    }
    let rows = build_rows(&snap, access);
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

/// The real write-access probe: a missing directory is `Missing` (that is
/// [`super::gone`]'s concern, not ours), an existing one is `Writable` or
/// `ReadOnly` per `access(path, W_OK)`.
fn access(path: &str) -> Access {
    if !Path::new(path).exists() {
        return Access::Missing;
    }
    let ok = CString::new(path)
        .map(|c| unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 })
        .unwrap_or(false);
    if ok {
        Access::Writable
    } else {
        Access::ReadOnly
    }
}

/// One row per live pane whose (non-empty) working directory exists but is not
/// writable. `probe` is injected so the check is testable without touching the
/// filesystem; each unique path is probed at most once. Ordered by location.
fn build_rows<F: Fn(&str) -> Access>(snap: &Snapshot, probe: F) -> Vec<Row> {
    let mut cache: HashMap<&str, Access> = HashMap::new();
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead && !p.path.is_empty())
        .filter(|p| {
            let a = *cache
                .entry(p.path.as_str())
                .or_insert_with(|| probe(&p.path));
            a == Access::ReadOnly
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

    /// A fake probe: paths under "/ro" are read-only, "/missing" is missing,
    /// everything else is writable.
    fn fake(path: &str) -> Access {
        if path.starts_with("/ro") {
            Access::ReadOnly
        } else if path == "/missing" {
            Access::Missing
        } else {
            Access::Writable
        }
    }

    #[test]
    fn only_read_only_dirs_are_reported() {
        let sn = snap(vec![
            pane("%1", 0, "/home/u"),  // writable
            pane("%2", 1, "/ro/tree"), // read-only
            pane("%3", 2, "/missing"), // missing -> gone's job, not ours
        ]);
        let rows = build_rows(&sn, fake);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%2");
        assert_eq!(rows[0].path, "/ro/tree");
    }

    #[test]
    fn dead_and_pathless_panes_are_skipped() {
        let mut dead = pane("%2", 1, "/ro/x");
        dead.dead = true;
        let sn = snap(vec![pane("%1", 0, ""), dead, pane("%3", 2, "/ro/y")]);
        let rows = build_rows(&sn, fake);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%3");
    }

    #[test]
    fn all_writable_renders_header_only() {
        let sn = snap(vec![pane("%1", 0, "/home/u")]);
        let rows = build_rows(&sn, fake);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("PANE") && s.contains("PATH"));
    }

    #[test]
    fn json_carries_path() {
        let sn = snap(vec![pane("%1", 0, "/ro/tree")]);
        let rows = build_rows(&sn, fake);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%1");
        assert_eq!(v[0]["path"], "/ro/tree");
    }
}
