//! `ztmux marks` — the marked pane(s).
//!
//! `select-pane -m` sets the marked pane: the implicit source that `join-pane`,
//! `swap-pane`, and `move-pane` use when no `-s` is given. It is invisible unless
//! you happen to be looking at the marked window, so it is easy to forget one is
//! set. `marks` lists the marked pane and where it is, so the pending source for
//! the next join/swap is always one command away. (tmux keeps at most one mark,
//! so this is normally zero or one row.) With `-o json` / `--json` it emits the
//! same rows as a machine-readable array, sorted by location.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a marked pane and where it lives.
struct Row {
    id: String,
    location: String,
    command: String,
    path: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux marks: {e}");
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

/// One row per live marked pane, ordered by location. Normally zero or one row
/// (tmux keeps a single mark), but the code does not assume that.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead && p.marked)
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

    fn pane(id: &str, sess: &str, win: i64, idx: i64, marked: bool) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: "zsh".into(),
            path: "/w".into(),
            marked,
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
    fn only_marked_panes_are_listed() {
        let rows = build_rows(&snap(vec![
            pane("%0", "a", 0, 0, false),
            pane("%1", "a", 1, 0, true),
        ]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%1");
        assert_eq!(rows[0].location, "a:1.0");
    }

    #[test]
    fn no_marks_yields_no_rows() {
        let rows = build_rows(&snap(vec![pane("%0", "a", 0, 0, false)]));
        assert!(rows.is_empty());
    }

    #[test]
    fn dead_marked_pane_is_skipped() {
        let mut dead = pane("%1", "a", 1, 0, true);
        dead.dead = true;
        assert!(build_rows(&snap(vec![dead])).is_empty());
    }

    #[test]
    fn rows_sorted_by_location() {
        let rows = build_rows(&snap(vec![
            pane("%9", "z", 5, 0, true),
            pane("%1", "a", 0, 0, true),
        ]));
        assert_eq!(rows[0].location, "a:0.0");
        assert_eq!(rows[1].location, "z:5.0");
    }

    #[test]
    fn json_carries_marked_pane_fields() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 1, 0, true)]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%1");
        assert_eq!(v[0]["location"], "a:1.0");
    }
}
