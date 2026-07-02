//! `ztmux user` — the owner of every pane's foreground process.
//!
//! For each live pane it asks `ps` for the user the pane's process runs as and
//! reports it, grouped by user. On a single-user box this is uniform, but on a
//! shared host or a server it answers "who is running what, and where", and it
//! catches the pane you left in a `sudo -s` or a `root` shell among your own.
//! Where [`super::ps`] prints CPU/memory and [`super::elapsed`] prints runtime,
//! `user` prints ownership — the one attribute that matters when a box is not
//! yours alone. One `ps` call covers every pid. With `-o json` / `--json` it
//! emits the same rows as a machine-readable array, sorted by user then location.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::process::Command;

use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a pane and the user its process runs as.
struct Row {
    id: String,
    location: String,
    command: String,
    user: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux user: {e}");
        return 1;
    }
    let owners = gather(&pids(&snap));
    let rows = build_rows(&snap, &owners);
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

/// One `ps -o pid=,user= -p <pids>` call for every pid; parse into a
/// pid→username map. Empty on failure so the extension degrades to no rows.
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
        .args(["-o", "pid=,user=", "-p", &list])
        .output()
    else {
        return HashMap::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut map = HashMap::new();
    for line in text.lines() {
        if let Some((pid, user)) = parse_ps_line(line) {
            map.insert(pid, user);
        }
    }
    map
}

/// Parse one `pid user` line into `(pid, username)`.
fn parse_ps_line(line: &str) -> Option<(i64, String)> {
    let mut it = line.split_whitespace();
    let pid: i64 = it.next()?.parse().ok()?;
    let user = it.next()?.to_string();
    Some((pid, user))
}

/// One row per live pane whose pid resolved to an owner, grouped by user then
/// location.
fn build_rows(snap: &Snapshot, owners: &HashMap<i64, String>) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let user = owners.get(&p.pid)?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                command: p.command.clone(),
                user: user.clone(),
            })
        })
        .collect();
    rows.sort_by(|a, b| a.user.cmp(&b.user).then(a.location.cmp(&b.location)));
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
                "{:<12} {:<8} {:<16} {}",
                "USER", "PANE", "LOCATION", "COMMAND"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<12} {:<8} {:<16} {}\n",
            r.user, r.id, r.location, r.command,
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
                "user": r.user,
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
    fn parses_pid_and_username() {
        assert_eq!(
            parse_ps_line(" 1234 root"),
            Some((1234, "root".to_string()))
        );
        assert_eq!(parse_ps_line("42 jane"), Some((42, "jane".to_string())));
        assert_eq!(parse_ps_line("nope"), None);
    }

    fn pane(id: &str, idx: i64, pid: i64) -> Pane {
        Pane {
            id: id.into(),
            session: "a".into(),
            window: 0,
            index: idx,
            pid,
            command: "cmd".into(),
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
    fn rows_grouped_by_user_then_location() {
        let sn = snap(vec![
            pane("%1", 0, 10),
            pane("%2", 1, 20),
            pane("%3", 2, 30),
        ]);
        let mut owners: HashMap<i64, String> = HashMap::new();
        owners.insert(10, "user".into());
        owners.insert(20, "root".into());
        owners.insert(30, "root".into());
        let rows = build_rows(&sn, &owners);
        // "root" sorts before "user"; within root, location order.
        assert_eq!(rows[0].user, "root");
        assert_eq!(rows[0].location, "a:0.1");
        assert_eq!(rows[1].location, "a:0.2");
        assert_eq!(rows[2].user, "user");
    }

    #[test]
    fn panes_without_a_resolved_owner_are_omitted() {
        let sn = snap(vec![pane("%1", 0, 10), pane("%2", 1, 20)]);
        let mut owners: HashMap<i64, String> = HashMap::new();
        owners.insert(10, "user".into());
        // pid 20 unresolved.
        let rows = build_rows(&sn, &owners);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%1");
    }

    #[test]
    fn json_carries_user() {
        let sn = snap(vec![pane("%1", 0, 10)]);
        let mut owners: HashMap<i64, String> = HashMap::new();
        owners.insert(10, "root".into());
        let rows = build_rows(&sn, &owners);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["user"], "root");
        assert_eq!(v[0]["id"], "%1");
    }
}
