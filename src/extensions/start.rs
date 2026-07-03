//! `ztmux start` — the command line each pane's root process was launched with.
//!
//! A pane's `pane_current_command` (what [`super::ps`] and most views show) is
//! the *foreground* process name — `vim`, `zsh`. `start` shows something
//! different: the full command line of the pane's *root* process (its
//! `pane_pid`), i.e. what the pane was originally launched as. For a shell pane
//! that is the shell invocation (`-zsh`); for a pane started with
//! `new-window 'ssh host'` it is that whole command with its arguments. It
//! answers "what did I open this pane to do", which the truncated foreground
//! name loses. One `ps` call covers every pid. With `-o json` / `--json` it emits
//! the same rows as a machine-readable array, sorted by location.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::process::Command;

use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a pane, its current foreground command, and its launch line.
struct Row {
    id: String,
    location: String,
    current: String, // pane_current_command (foreground name)
    start: String,   // full command line of the pane's root process
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux start: {e}");
        return 1;
    }
    let cmds = gather(&pids(&snap));
    let rows = build_rows(&snap, &cmds);
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

/// The distinct pids of every live pane with a real pid.
fn pids(snap: &Snapshot) -> Vec<i64> {
    let mut v: Vec<i64> = snap
        .panes
        .iter()
        .filter(|p| !p.dead && p.pid > 0)
        .map(|p| p.pid)
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// One `ps -o pid=,command= -p <pids>` call for every pid; parse into a
/// pid→command-line map. Empty on failure so the extension degrades to no rows.
fn gather(pids: &[i64]) -> HashMap<i64, String> {
    if pids.is_empty() {
        return HashMap::new();
    }
    let list = pids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let Ok(out) = Command::new("ps")
        .args(["-o", "pid=,command=", "-p", &list])
        .output()
    else {
        return HashMap::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut map = HashMap::new();
    for line in text.lines() {
        if let Some((pid, cmd)) = parse_ps_line(line) {
            map.insert(pid, cmd);
        }
    }
    map
}

/// Parse one `pid command…` line into `(pid, command_line)`. The command line
/// keeps its spaces, so only the first whitespace run (before the pid) is split.
fn parse_ps_line(line: &str) -> Option<(i64, String)> {
    let trimmed = line.trim_start();
    let (pid_str, rest) = trimmed.split_once(char::is_whitespace)?;
    let pid: i64 = pid_str.parse().ok()?;
    let cmd = rest.trim();
    if cmd.is_empty() {
        return None;
    }
    Some((pid, cmd.to_string()))
}

/// One row per live pane whose root pid resolved to a command line, ordered by
/// location for a stable table.
fn build_rows(snap: &Snapshot, cmds: &HashMap<i64, String>) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let start = cmds.get(&p.pid)?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                current: p.command.clone(),
                start: start.clone(),
            })
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
                "PANE", "LOCATION", "CURRENT", "START"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<12} {}\n",
            r.id, r.location, r.current, r.start,
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
                "current": r.current,
                "start": r.start,
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

    #[test]
    fn parse_keeps_the_full_command_line() {
        assert_eq!(
            parse_ps_line(" 1234 ssh -p 22 host.example"),
            Some((1234, "ssh -p 22 host.example".to_string()))
        );
        assert_eq!(parse_ps_line("42 -zsh"), Some((42, "-zsh".to_string())));
    }

    #[test]
    fn parse_rejects_malformed_lines() {
        assert_eq!(parse_ps_line("nopid"), None);
        assert_eq!(parse_ps_line("1234 "), None); // no command
        assert_eq!(parse_ps_line(""), None);
    }

    fn pane(id: &str, idx: i64, pid: i64, cmd: &str) -> Pane {
        Pane {
            id: id.into(),
            session: "a".into(),
            window: 0,
            index: idx,
            pid,
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
    fn joins_current_and_start_and_omits_unresolved() {
        let sn = snap(vec![pane("%1", 0, 10, "ssh"), pane("%2", 1, 20, "zsh")]);
        let mut cmds: HashMap<i64, String> = HashMap::new();
        cmds.insert(10, "ssh -p 22 host".into());
        // pid 20 unresolved -> omitted.
        let rows = build_rows(&sn, &cmds);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].current, "ssh");
        assert_eq!(rows[0].start, "ssh -p 22 host");
    }

    #[test]
    fn json_carries_current_and_start() {
        let sn = snap(vec![pane("%1", 0, 10, "vim")]);
        let mut cmds: HashMap<i64, String> = HashMap::new();
        cmds.insert(10, "vim src/main.rs".into());
        let rows = build_rows(&sn, &cmds);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["current"], "vim");
        assert_eq!(v[0]["start"], "vim src/main.rs");
    }
}
