//! `ztmux cmd` — a histogram of the commands running across every pane.
//!
//! Where [`super::ps`] prints one row *per pane* and [`super::cwd`] groups panes
//! by *directory*, `cmd` collapses the server the other way: it counts how many
//! live panes are running each command and ranks the commands by that count,
//! busiest first — the "what am I actually running, and how much of it" view.
//! It answers "how many shells / editors / builds do I have open" in one line
//! each. With `-o json` / `--json` it emits the command, the pane count, and
//! every pane location as a machine-readable array.

use std::io::IsTerminal;

use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a command with the panes running it.
struct Row {
    command: String,
    count: usize,
    locations: Vec<String>,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux cmd: {e}");
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

/// Count live (non-dead) panes per command, biggest count first. A
/// [`std::collections::BTreeMap`] gives a deterministic command order; a stable
/// count-descending sort then puts the most-run command on top while leaving
/// equally-run commands in command order. Panes carrying no command are skipped.
fn build_rows(snap: &Snapshot) -> Vec<Row> {
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in &snap.panes {
        if p.dead || p.command.is_empty() {
            continue;
        }
        buckets
            .entry(p.command.clone())
            .or_default()
            .push(location(p));
    }
    let mut rows: Vec<Row> = buckets
        .into_iter()
        .map(|(command, mut locations)| {
            locations.sort();
            Row {
                command,
                count: locations.len(),
                locations,
            }
        })
        .collect();
    // Most-run command first; ties keep the map's command order (stable sort).
    rows.sort_by_key(|r| std::cmp::Reverse(r.count));
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
        paint(&format!("{:>5} {}", "PANES", "COMMAND"), "1")
    ));
    for r in rows {
        out.push_str(&format!("{:>5} {}\n", r.count, r.command));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "command": r.command,
                "panes": r.count,
                "locations": r.locations,
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
    fn most_run_command_sorts_first() {
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, "zsh"),
            pane("%2", "a", 0, 1, "zsh"),
            pane("%3", "a", 0, 2, "vim"),
            pane("%4", "a", 0, 3, "zsh"),
        ]));
        assert_eq!(rows[0].command, "zsh");
        assert_eq!(rows[0].count, 3);
        assert_eq!(rows[1].command, "vim");
    }

    #[test]
    fn equal_counts_break_ties_by_command_name() {
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, "vim"),
            pane("%2", "a", 0, 1, "cargo"),
        ]));
        assert_eq!(rows[0].command, "cargo");
        assert_eq!(rows[1].command, "vim");
    }

    #[test]
    fn dead_and_commandless_panes_are_skipped() {
        let mut dead = pane("%2", "a", 0, 1, "zsh");
        dead.dead = true;
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, "zsh"),
            dead,
            pane("%3", "a", 0, 2, ""),
        ]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].count, 1);
    }

    #[test]
    fn locations_are_collected_and_sorted() {
        let rows = build_rows(&snap(vec![
            pane("%2", "a", 0, 1, "zsh"),
            pane("%1", "a", 0, 0, "zsh"),
        ]));
        assert_eq!(rows[0].locations, vec!["a:0.0", "a:0.1"]);
    }

    #[test]
    fn text_renders_header_count_and_command() {
        let rows = build_rows(&snap(vec![pane("%1", "a", 0, 0, "vim")]));
        let s = render_text(&rows, false);
        assert!(s.contains("PANES") && s.contains("COMMAND"));
        assert!(s.lines().any(|l| l.contains("vim")));
    }

    #[test]
    fn json_carries_command_count_and_locations() {
        let rows = build_rows(&snap(vec![
            pane("%1", "a", 0, 0, "zsh"),
            pane("%2", "a", 0, 1, "zsh"),
        ]));
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["command"], "zsh");
        assert_eq!(v[0]["panes"], 2);
        assert_eq!(v[0]["locations"][1], "a:0.1");
    }
}
