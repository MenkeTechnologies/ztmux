//! `ztmux elapsed` — how long each pane's foreground process has been running.
//!
//! For every live pane it asks `ps` for the elapsed running time of the pane's
//! process and reports it, longest-running first — the "what has been going the
//! longest" view that surfaces the three-hour build you forgot about or the ssh
//! session open since this morning. Where [`super::usage`] ranks by CPU/memory
//! (how *hard* a process is working) and [`super::ps`] lists the process table,
//! `elapsed` ranks by wall-clock age (how *long* it has been working). One `ps`
//! call covers every pid. With `-o json` / `--json` it emits the same rows
//! (elapsed as raw seconds) as a machine-readable array.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::process::Command;

use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a pane and how long its process has been running.
struct Row {
    id: String,
    location: String,
    command: String,
    seconds: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux elapsed: {e}");
        return 1;
    }
    let ages = gather(&pids(&snap));
    let rows = build_rows(&snap, &ages);
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

/// One `ps -o pid=,etime= -p <pids>` call for every pid; parse into a
/// pid→seconds map. Empty on failure so the extension degrades to no rows.
fn gather(pids: &[i64]) -> HashMap<i64, i64> {
    if pids.is_empty() {
        return HashMap::new();
    }
    let list = pids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let Ok(out) = Command::new("ps")
        .args(["-o", "pid=,etime=", "-p", &list])
        .output()
    else {
        return HashMap::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut map = HashMap::new();
    for line in text.lines() {
        if let Some((pid, secs)) = parse_ps_line(line) {
            map.insert(pid, secs);
        }
    }
    map
}

/// Parse one `pid etime` line into `(pid, seconds)`.
fn parse_ps_line(line: &str) -> Option<(i64, i64)> {
    let mut it = line.split_whitespace();
    let pid: i64 = it.next()?.parse().ok()?;
    let secs = parse_etime(it.next()?)?;
    Some((pid, secs))
}

/// Parse a `ps` `etime` field — `[[DD-]hh:]mm:ss` — into total seconds.
fn parse_etime(s: &str) -> Option<i64> {
    let (days, hms) = match s.split_once('-') {
        Some((d, rest)) => (d.parse::<i64>().ok()?, rest),
        None => (0, s),
    };
    let parts: Vec<&str> = hms.split(':').collect();
    let (h, m, sec): (i64, i64, i64) = match parts.as_slice() {
        [h, m, s] => (h.parse().ok()?, m.parse().ok()?, s.parse().ok()?),
        [m, s] => (0, m.parse().ok()?, s.parse().ok()?),
        _ => return None,
    };
    Some(days * 86_400 + h * 3_600 + m * 60 + sec)
}

/// Format a duration in seconds as up to two compact units (e.g. `3d2h`,
/// `4h12m`, `5m30s`, `42s`).
fn human(secs: i64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3_600;
    let m = (secs % 3_600) / 60;
    let s = secs % 60;
    if d > 0 {
        format!("{d}d{h}h")
    } else if h > 0 {
        format!("{h}h{m}m")
    } else if m > 0 {
        format!("{m}m{s}s")
    } else {
        format!("{s}s")
    }
}

/// One row per live pane whose pid resolved to an elapsed time, longest-running
/// first; ties break by location.
fn build_rows(snap: &Snapshot, ages: &HashMap<i64, i64>) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let secs = *ages.get(&p.pid)?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                command: p.command.clone(),
                seconds: secs,
            })
        })
        .collect();
    rows.sort_by(|a, b| b.seconds.cmp(&a.seconds).then(a.location.cmp(&b.location)));
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
                "PANE", "LOCATION", "COMMAND", "ELAPSED"
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
            human(r.seconds),
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
                "seconds": r.seconds,
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
    fn parse_etime_mm_ss() {
        assert_eq!(parse_etime("05:30"), Some(330));
    }

    #[test]
    fn parse_etime_hh_mm_ss() {
        assert_eq!(parse_etime("02:00:00"), Some(7_200));
    }

    #[test]
    fn parse_etime_days() {
        assert_eq!(
            parse_etime("3-04:05:06"),
            Some(3 * 86_400 + 4 * 3_600 + 5 * 60 + 6)
        );
    }

    #[test]
    fn parse_etime_rejects_garbage() {
        assert_eq!(parse_etime("nope"), None);
        assert_eq!(parse_etime(""), None);
    }

    #[test]
    fn parse_ps_line_pairs_pid_and_seconds() {
        assert_eq!(parse_ps_line(" 1234 01:00"), Some((1234, 60)));
    }

    #[test]
    fn human_uses_two_units() {
        assert_eq!(human(42), "42s");
        assert_eq!(human(330), "5m30s");
        assert_eq!(human(7_260), "2h1m");
        assert_eq!(human(90_000), "1d1h");
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
    fn longest_running_sorts_first_and_missing_pids_omitted() {
        let sn = snap(vec![
            pane("%1", 0, 10),
            pane("%2", 1, 20),
            pane("%3", 2, 30),
        ]);
        let mut ages: HashMap<i64, i64> = HashMap::new();
        ages.insert(10, 60);
        ages.insert(20, 7_200);
        // pid 30 has no elapsed entry -> omitted.
        let rows = build_rows(&sn, &ages);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "%2");
        assert_eq!(rows[0].seconds, 7_200);
        assert_eq!(rows[1].id, "%1");
    }

    #[test]
    fn pids_are_deduped_and_dead_skipped() {
        let mut dead = pane("%3", 2, 99);
        dead.dead = true;
        let sn = snap(vec![pane("%1", 0, 10), pane("%2", 1, 10), dead]);
        assert_eq!(pids(&sn), vec![10]);
    }

    #[test]
    fn json_carries_seconds() {
        let sn = snap(vec![pane("%1", 0, 10)]);
        let mut ages: HashMap<i64, i64> = HashMap::new();
        ages.insert(10, 3_600);
        let rows = build_rows(&sn, &ages);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["id"], "%1");
        assert_eq!(v[0]["seconds"], 3_600);
    }
}
