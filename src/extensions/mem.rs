//! `ztmux mem` — panes ranked by the resident memory of their process.
//!
//! Where [`super::usage`] aggregates memory *per session* and [`super::ps`]
//! prints the whole process table sorted by CPU, `mem` is the RAM-focused board:
//! every live pane's process ranked by resident set size, biggest first — the
//! "which pane is eating memory" view, the companion to [`super::size`] (screen
//! geometry) and [`super::history`] (scrollback) for a single-metric ranking.
//! Process stats come from the shared [`super::procstat`] `ps` call. With
//! `-o json` / `--json` it emits the same rows (RSS in kilobytes) as a
//! machine-readable array.

use std::io::IsTerminal;

use super::procstat::{ProcStat, fmt_rss, gather};
use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a pane and the resident memory of its process.
struct Row {
    id: String,
    location: String,
    command: String,
    rss_kb: u64,
    mem: f32,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux mem: {e}");
        return 1;
    }
    let stats = gather(&pids(&snap));
    let rows = build_rows(&snap, &stats);
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

/// One row per live pane whose pid resolved to process stats, largest resident
/// set first; ties break by location.
fn build_rows(snap: &Snapshot, stats: &std::collections::HashMap<i64, ProcStat>) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let st = stats.get(&p.pid)?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                command: p.command.clone(),
                rss_kb: st.rss_kb,
                mem: st.mem,
            })
        })
        .collect();
    rows.sort_by(|a, b| b.rss_kb.cmp(&a.rss_kb).then(a.location.cmp(&b.location)));
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
                "{:>9} {:>6} {:<8} {:<16} {}",
                "RSS", "%MEM", "PANE", "LOCATION", "COMMAND"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:>9} {:>6.1} {:<8} {:<16} {}\n",
            fmt_rss(r.rss_kb),
            r.mem,
            r.id,
            r.location,
            r.command,
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
                "rss_kb": r.rss_kb,
                "mem": r.mem,
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
    use std::collections::HashMap;

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

    fn stat(rss_kb: u64, mem: f32) -> ProcStat {
        ProcStat {
            rss_kb,
            mem,
            ..Default::default()
        }
    }

    #[test]
    fn largest_rss_sorts_first_and_missing_pids_omitted() {
        let sn = snap(vec![
            pane("%1", 0, 10),
            pane("%2", 1, 20),
            pane("%3", 2, 30),
        ]);
        let mut stats: HashMap<i64, ProcStat> = HashMap::new();
        stats.insert(10, stat(2_048, 0.5));
        stats.insert(20, stat(65_536, 8.0));
        // pid 30 has no stat -> omitted.
        let rows = build_rows(&sn, &stats);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "%2");
        assert_eq!(rows[0].rss_kb, 65_536);
        assert_eq!(rows[1].id, "%1");
    }

    #[test]
    fn pids_deduped_and_dead_skipped() {
        let mut dead = pane("%3", 2, 99);
        dead.dead = true;
        let sn = snap(vec![pane("%1", 0, 10), pane("%2", 1, 10), dead]);
        assert_eq!(pids(&sn), vec![10]);
    }

    #[test]
    fn text_renders_human_rss() {
        let sn = snap(vec![pane("%1", 0, 10)]);
        let mut stats: HashMap<i64, ProcStat> = HashMap::new();
        stats.insert(10, stat(65_536, 8.0));
        let rows = build_rows(&sn, &stats);
        let s = render_text(&rows, false);
        assert!(s.contains("RSS") && s.contains("%MEM"));
        assert!(s.lines().any(|l| l.contains("64.0M")));
    }

    #[test]
    fn json_carries_rss_kb_and_mem() {
        let sn = snap(vec![pane("%1", 0, 10)]);
        let mut stats: HashMap<i64, ProcStat> = HashMap::new();
        stats.insert(10, stat(4_096, 1.5));
        let rows = build_rows(&sn, &stats);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["rss_kb"], 4_096);
        assert_eq!(v[0]["id"], "%1");
    }
}
