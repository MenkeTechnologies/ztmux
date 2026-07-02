//! `ztmux busy` — panes actually running a program, not idle at a prompt.
//!
//! The exact complement of [`super::shells`]: where `shells` lists the panes
//! sitting at a bare shell prompt, `busy` lists the live panes running anything
//! else — an editor, a build, an `ssh`, a nested multiplexer. It is the "where
//! is work actually happening right now" view, the pipeable primitive behind
//! "which of my panes has a job going". The shell set is shared with `shells`
//! (via [`super::shells::is_shell`]) so the two views partition the live panes
//! exactly. It is matched on the pane's foreground command
//! (`pane_current_command`). With `-o json` / `--json` it emits the same rows as
//! a machine-readable array; a server where every pane is idle prints just the
//! header.

use std::io::IsTerminal;

use super::shells::is_shell;
use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a pane running a foreground program.
struct Row {
    id: String,
    location: String,
    command: String,
    path: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux busy: {e}");
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

/// One row per live pane whose foreground command is not a shell (and not
/// empty), ordered by location for a stable, greppable table.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead && !p.command.is_empty() && !is_shell(&p.command))
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

    fn pane(id: &str, sess: &str, win: i64, idx: i64, cmd: &str) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: cmd.into(),
            path: "/src".into(),
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
    fn only_non_shell_panes_are_reported() {
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, "zsh"),
            pane("%2", "a", 0, 1, "vim"),
            pane("%3", "a", 0, 2, "bash"),
            pane("%4", "a", 0, 3, "cargo"),
        ]));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].command, "vim");
        assert_eq!(rows[1].command, "cargo");
    }

    #[test]
    fn dead_and_commandless_panes_are_skipped() {
        let mut dead = pane("%2", "a", 0, 1, "vim");
        dead.dead = true;
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, "cargo"),
            dead,
            pane("%3", "a", 0, 2, ""),
        ]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "cargo");
    }

    // busy and shells must partition the live, commanded panes exactly.
    #[test]
    fn busy_is_the_exact_complement_of_shells() {
        use super::super::shells::is_shell;
        let panes = vec![
            pane("%1", "a", 0, 0, "zsh"),
            pane("%2", "a", 0, 1, "vim"),
            pane("%3", "a", 0, 2, "ssh"),
            pane("%4", "a", 0, 3, "fish"),
        ];
        let busy = build_rows(&snap(panes.clone()));
        let shell_count = panes.iter().filter(|p| is_shell(&p.command)).count();
        assert_eq!(busy.len() + shell_count, panes.len());
    }

    #[test]
    fn no_busy_panes_renders_header_only() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, "zsh")]));
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("PANE") && s.contains("COMMAND"));
    }

    #[test]
    fn json_carries_all_fields() {
        let rows = build_rows(&snap(vec![pane("%2", "a", 0, 1, "vim")]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%2");
        assert_eq!(v[0]["location"], "a:0.1");
        assert_eq!(v[0]["command"], "vim");
        assert_eq!(v[0]["path"], "/src");
    }
}
