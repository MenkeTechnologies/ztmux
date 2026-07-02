//! `ztmux dead` — every dead pane still held open in a window.
//!
//! A pane goes *dead* when its process exits while `remain-on-exit` keeps the
//! pane in place. Where [`super::prune`] *reclaims* dead panes and
//! [`super::respawn`] *revives* them — both mutating actions — `dead` only
//! *reports* them: the read-only inventory you look at before deciding whether
//! to prune or respawn. Each row shows the pane, its location, the command that
//! was running, its last pid, and its working directory. With `-o json` /
//! `--json` it emits the same rows as a machine-readable array; an empty server
//! (no dead panes) prints just the header.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a single dead pane.
struct Row {
    id: String,
    location: String,
    command: String,
    pid: i64,
    path: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux dead: {e}");
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

/// One row per dead pane, ordered by location for a stable, greppable table.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| p.dead)
        .map(|p| Row {
            id: p.id.clone(),
            location: location(p),
            command: p.command.clone(),
            pid: p.pid,
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
                "{:<8} {:<16} {:<12} {:>8} {}",
                "PANE", "LOCATION", "COMMAND", "PID", "PATH"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<12} {:>8} {}\n",
            r.id, r.location, r.command, r.pid, r.path,
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
                "pid": r.pid,
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

    fn pane(id: &str, sess: &str, win: i64, idx: i64, cmd: &str, dead: bool) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: cmd.into(),
            path: "/src".into(),
            pid: 123,
            dead,
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
    fn only_dead_panes_are_reported() {
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, "zsh", false),
            pane("%2", "a", 0, 1, "make", true),
            pane("%3", "a", 0, 2, "vim", false),
        ]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%2");
        assert_eq!(rows[0].command, "make");
    }

    #[test]
    fn rows_sorted_by_location() {
        let rows = build_rows(&snap(vec![
            pane("%9", "z", 9, 0, "make", true),
            pane("%1", "a", 0, 0, "make", true),
        ]));
        assert_eq!(rows[0].location, "a:0.0");
        assert_eq!(rows[1].location, "z:9.0");
    }

    #[test]
    fn no_dead_panes_renders_header_only() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, "zsh", false)]));
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("PANE") && s.contains("PATH"));
    }

    #[test]
    fn text_carries_pid_and_path() {
        let rows = build_rows(&snap(vec![pane("%2", "a", 0, 1, "make", true)]));
        let s = render_text(&rows, false);
        assert!(s.lines().any(|l| l.contains("123") && l.contains("/src")));
    }

    #[test]
    fn json_carries_all_dead_pane_fields() {
        let rows = build_rows(&snap(vec![pane("%2", "a", 0, 1, "make", true)]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%2");
        assert_eq!(v[0]["location"], "a:0.1");
        assert_eq!(v[0]["command"], "make");
        assert_eq!(v[0]["pid"], 123);
        assert_eq!(v[0]["path"], "/src");
    }
}
