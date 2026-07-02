//! `ztmux tty` — map every live pane to its terminal device.
//!
//! Each pane owns a pseudo-terminal (`/dev/ttysNNN`); the device path is what
//! you need to write straight to a pane (`echo hi > $(…)`), or to correlate a
//! pane with system tools like `ps -t`, `w`, or `lsof`. No other extension
//! surfaces it: [`super::ps`] reports the pane's *process*, [`super::info`]
//! inspects one pane deeply. `tty` is the flat, pipeable pane→device table,
//! sorted by location. Dead panes (no live terminal) are skipped. With
//! `-o json` / `--json` it emits the same rows as a machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a pane and its terminal device path.
struct Row {
    id: String,
    location: String,
    command: String,
    tty: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux tty: {e}");
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

/// One row per live pane, ordered by location for a stable, greppable table.
/// Dead panes are skipped — their terminal is gone.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead)
        .map(|p| Row {
            id: p.id.clone(),
            location: location(p),
            command: p.command.clone(),
            tty: p.tty.clone(),
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
                "PANE", "LOCATION", "COMMAND", "TTY"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<12} {}\n",
            r.id,
            r.location,
            r.command,
            if r.tty.is_empty() { "-" } else { &r.tty },
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
                "tty": r.tty,
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

    fn pane(id: &str, sess: &str, win: i64, idx: i64, cmd: &str, tty: &str) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: cmd.into(),
            tty: tty.into(),
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
    fn rows_sorted_by_location() {
        let rows = build_rows(&snap(vec![
            pane("%9", "z", 5, 0, "zsh", "/dev/ttys009"),
            pane("%1", "a", 0, 0, "zsh", "/dev/ttys001"),
        ]));
        assert_eq!(rows[0].location, "a:0.0");
        assert_eq!(rows[0].tty, "/dev/ttys001");
        assert_eq!(rows[1].location, "z:5.0");
    }

    #[test]
    fn dead_panes_are_skipped() {
        let mut dead = pane("%2", "a", 0, 1, "zsh", "/dev/ttys002");
        dead.dead = true;
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, "zsh", "/dev/ttys001"),
            dead,
        ]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%1");
    }

    #[test]
    fn empty_tty_renders_as_dash() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, "zsh", "")]));
        let s = render_text(&rows, false);
        let line = s.lines().find(|l| l.contains("%1")).unwrap();
        assert!(line.trim_end().ends_with(" -"));
    }

    #[test]
    fn json_carries_device_path() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, "nvim", "/dev/ttys003")]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%1");
        assert_eq!(v[0]["command"], "nvim");
        assert_eq!(v[0]["tty"], "/dev/ttys003");
    }
}
