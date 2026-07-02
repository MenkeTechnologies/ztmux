//! `ztmux shells` — panes sitting at a bare shell prompt.
//!
//! A pane whose foreground command is an interactive shell (`zsh`, `bash`,
//! `fish`, …) is idle at a prompt — nothing is running in it, so it is a pane you
//! can drop into and reuse. Where [`super::cmd`] counts *every* command and
//! [`super::nested`] isolates the multiplexer panes, `shells` isolates the free,
//! at-prompt panes — the "where can I go type something" view. It is matched on
//! the pane's foreground command (`pane_current_command`): a shell running a
//! foreground job reports that job's name instead, so it correctly drops out.
//! With `-o json` / `--json` it emits the same rows as a machine-readable array;
//! a server with no idle shells prints just the header.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, poll};

/// Foreground commands that are interactive shells (a pane idle at a prompt).
const SHELLS: &[&str] = &[
    "sh", "bash", "zsh", "zshrs", "fish", "dash", "ksh", "mksh", "tcsh", "csh", "ash", "elvish",
    "nu", "xonsh",
];

/// One output row: a pane idle at a shell prompt.
struct Row {
    id: String,
    location: String,
    shell: String,
    path: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux shells: {e}");
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

/// `true` when a pane's foreground command is an interactive shell. Matched on
/// the exact command name so a program merely *containing* a shell name (say a
/// hypothetical `bashtop`) is not counted as an idle shell. Shared with
/// [`super::busy`], which reports the exact complement (panes *not* at a shell).
pub(crate) fn is_shell(command: &str) -> bool {
    SHELLS.contains(&command)
}

/// One row per live pane sitting at a shell prompt, ordered by location for a
/// stable, greppable table.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead && is_shell(&p.command))
        .map(|p| Row {
            id: p.id.clone(),
            location: location(p),
            shell: p.command.clone(),
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
                "{:<8} {:<16} {:<10} {}",
                "PANE", "LOCATION", "SHELL", "PATH"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<10} {}\n",
            r.id, r.location, r.shell, r.path,
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
                "shell": r.shell,
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
    fn known_shells_match_exactly() {
        assert!(is_shell("zsh"));
        assert!(is_shell("bash"));
        assert!(is_shell("fish"));
        assert!(is_shell("zshrs"));
        // A program running in the shell is not a bare prompt.
        assert!(!is_shell("vim"));
        assert!(!is_shell("bashtop"));
    }

    #[test]
    fn only_shell_panes_are_reported() {
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, "zsh"),
            pane("%2", "a", 0, 1, "vim"),
            pane("%3", "a", 0, 2, "bash"),
        ]));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "%1");
        assert_eq!(rows[1].id, "%3");
    }

    #[test]
    fn dead_shell_panes_are_skipped() {
        let mut dead = pane("%2", "a", 0, 1, "zsh");
        dead.dead = true;
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, "zsh"), dead]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%1");
    }

    #[test]
    fn no_idle_shells_renders_header_only() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, "vim")]));
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("PANE") && s.contains("SHELL"));
    }

    #[test]
    fn json_carries_all_fields() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, "zsh")]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%1");
        assert_eq!(v[0]["location"], "a:0.0");
        assert_eq!(v[0]["shell"], "zsh");
        assert_eq!(v[0]["path"], "/src");
    }
}
