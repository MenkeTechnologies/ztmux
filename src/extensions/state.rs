//! `ztmux state` — panes whose process is in an abnormal state.
//!
//! Most pane processes are running (`R`) or sleeping (`S`) — healthy, and not
//! worth listing. `state` filters to the ones that are *not*: a zombie that was
//! never reaped, a process stopped by a signal, or one wedged in an
//! uninterruptible disk/kernel wait. Where [`super::ps`] prints every pane's
//! process with its state mixed in, `state` isolates just the panes that need a
//! look — the health filter. Process state comes from the shared
//! [`super::procstat`] `ps` call; the leading state code is mapped to a readable
//! label. Healthy panes (and panes with no process) are omitted. With `-o json`
//! / `--json` it emits the same rows as a machine-readable array.

use std::io::IsTerminal;

use super::procstat::{ProcStat, gather};
use super::tmux_query::{Pane, Snapshot, poll};

/// One output row: a pane whose process is in an abnormal state.
struct Row {
    id: String,
    location: String,
    command: String,
    code: String,
    label: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux state: {e}");
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

/// Map a `ps` state field to a readable label when it is *abnormal*, else
/// `None`. Only the leading code matters (trailing flags like `+`, `s`, `<`,
/// `l`, `N` are decorations); `R` (running), `S`/`I` (sleeping/idle) are healthy
/// and filtered out. Covers both Linux (`D`, `T`, `t`, `Z`, `X`) and BSD/macOS
/// (`U` uninterruptible) codes.
fn classify(state: &str) -> Option<&'static str> {
    match state.chars().next()? {
        'Z' | 'X' => Some("zombie"),
        'T' => Some("stopped"),
        't' => Some("traced"),
        'D' | 'U' => Some("uninterruptible"),
        _ => None,
    }
}

/// One row per live pane whose process is in an abnormal state, ordered by
/// location for a stable, greppable table.
fn build_rows(snap: &Snapshot, stats: &std::collections::HashMap<i64, ProcStat>) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .panes
        .iter()
        .filter(|p| !p.dead)
        .filter_map(|p| {
            let st = stats.get(&p.pid)?;
            let label = classify(&st.state)?;
            Some(Row {
                id: p.id.clone(),
                location: location(p),
                command: p.command.clone(),
                code: st.state.clone(),
                label: label.to_string(),
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
                "{:<8} {:<16} {:<12} {:<6} {}",
                "PANE", "LOCATION", "COMMAND", "CODE", "STATE"
            ),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:<12} {:<6} {}\n",
            r.id, r.location, r.command, r.code, r.label,
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
                "code": r.code,
                "state": r.label,
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

    #[test]
    fn healthy_states_classify_none() {
        assert_eq!(classify("R"), None);
        assert_eq!(classify("S"), None);
        assert_eq!(classify("S+"), None);
        assert_eq!(classify("I"), None);
        assert_eq!(classify(""), None);
    }

    #[test]
    fn abnormal_states_classify_to_labels() {
        assert_eq!(classify("Z"), Some("zombie"));
        assert_eq!(classify("T"), Some("stopped"));
        assert_eq!(classify("t"), Some("traced"));
        assert_eq!(classify("D"), Some("uninterruptible"));
        assert_eq!(classify("U"), Some("uninterruptible"));
        // Trailing flags are ignored.
        assert_eq!(classify("Z+"), Some("zombie"));
        assert_eq!(classify("D<"), Some("uninterruptible"));
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

    fn stat(state: &str) -> ProcStat {
        ProcStat {
            state: state.into(),
            ..Default::default()
        }
    }

    #[test]
    fn only_abnormal_panes_are_reported() {
        let sn = snap(vec![
            pane("%1", 0, 10),
            pane("%2", 1, 20),
            pane("%3", 2, 30),
        ]);
        let mut stats: HashMap<i64, ProcStat> = HashMap::new();
        stats.insert(10, stat("S")); // healthy
        stats.insert(20, stat("Z")); // zombie
        stats.insert(30, stat("R")); // healthy
        let rows = build_rows(&sn, &stats);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "%2");
        assert_eq!(rows[0].label, "zombie");
    }

    #[test]
    fn no_abnormal_panes_renders_header_only() {
        let sn = snap(vec![pane("%1", 0, 10)]);
        let mut stats: HashMap<i64, ProcStat> = HashMap::new();
        stats.insert(10, stat("S+"));
        let rows = build_rows(&sn, &stats);
        assert!(rows.is_empty());
        let s = render_text(&rows, false);
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("PANE") && s.contains("STATE"));
    }

    #[test]
    fn json_carries_code_and_state_label() {
        let sn = snap(vec![pane("%1", 0, 10)]);
        let mut stats: HashMap<i64, ProcStat> = HashMap::new();
        stats.insert(10, stat("T"));
        let rows = build_rows(&sn, &stats);
        let v: serde_json::Value = serde_json::from_str(&render_json(&rows)).unwrap();
        assert_eq!(v[0]["code"], "T");
        assert_eq!(v[0]["state"], "stopped");
    }
}
