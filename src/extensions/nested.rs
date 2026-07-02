//! `ztmux nested` — panes that are themselves running a terminal multiplexer.
//!
//! A pane whose foreground command is `tmux`, `ztmux`, `screen`, `zellij`,
//! `byobu`, `dvtm`, `abduco`, or `dtach` is a multiplexer running *inside* one of
//! this server's own panes — a multiplexer inside a multiplexer. It is matched on
//! the pane's foreground command (`pane_current_command`), so it catches a mux
//! started directly in a local pane; a remote mux reached over SSH shows up as
//! `ssh` (see [`super::ssh`]) rather than here. Nested muxes swallow the prefix
//! key and confuse `send-keys`/`capture-pane`, so it helps to see exactly where
//! they are. Where [`super::cmd`] counts every command, `nested` isolates just
//! the multiplexer panes. With `-o json` / `--json` it emits the same rows as a
//! machine-readable array; a server with none prints just the header.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, poll};

/// Foreground commands that are themselves terminal multiplexers.
const MULTIPLEXERS: &[&str] = &[
    "tmux", "ztmux", "screen", "zellij", "byobu", "dvtm", "abduco", "dtach",
];

/// One output row: a pane running a nested multiplexer.
struct Row {
    id: String,
    location: String,
    command: String,
    path: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux nested: {e}");
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

/// `true` when a pane's foreground command is a known multiplexer. Matched on
/// the exact command name (not a substring) so `tmuxinator` or `screenfetch`
/// are not mistaken for the real thing.
fn is_multiplexer(command: &str) -> bool {
    MULTIPLEXERS.contains(&command)
}

/// One row per live pane whose command is a multiplexer, ordered by location
/// for a stable, greppable table.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead && is_multiplexer(&p.command))
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
    fn known_multiplexers_match_exactly() {
        assert!(is_multiplexer("tmux"));
        assert!(is_multiplexer("ztmux"));
        assert!(is_multiplexer("screen"));
        assert!(is_multiplexer("zellij"));
        // Substring look-alikes must not match.
        assert!(!is_multiplexer("tmuxinator"));
        assert!(!is_multiplexer("screenfetch"));
        assert!(!is_multiplexer("zsh"));
    }

    #[test]
    fn only_multiplexer_panes_are_reported() {
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, "zsh"),
            pane("%2", "a", 0, 1, "tmux"),
            pane("%3", "a", 0, 2, "vim"),
            pane("%4", "a", 0, 3, "ztmux"),
        ]));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "%2");
        assert_eq!(rows[1].id, "%4");
    }

    #[test]
    fn dead_multiplexer_panes_are_skipped() {
        let mut dead = pane("%2", "a", 0, 1, "tmux");
        dead.dead = true;
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, "screen"), dead]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "screen");
    }

    #[test]
    fn no_nested_muxes_renders_header_only() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, "zsh")]));
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("PANE") && s.contains("COMMAND"));
    }

    #[test]
    fn json_carries_all_fields() {
        let rows = build_rows(&snap(vec![pane("%2", "a", 0, 1, "tmux")]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%2");
        assert_eq!(v[0]["location"], "a:0.1");
        assert_eq!(v[0]["command"], "tmux");
        assert_eq!(v[0]["path"], "/src");
    }
}
