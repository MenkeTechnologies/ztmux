//! `ztmux stats` — a one-shot summary report of the running server.
//!
//! Like [`super::tree`] it is a client subcommand built from the structured
//! `list-* -o json` query layer ([`super::tmux_query`]); it needs no linkage
//! against the server internals. It prints totals, the busiest session, the
//! largest window, dead panes, and a per-command histogram — a "report" to
//! complement the interactive `dashboard` and the `tree` dump. With `-o json`
//! / `--json` it emits the same numbers as a machine-readable object.

use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::time::{SystemTime, UNIX_EPOCH};

use super::tmux_query::{Snapshot, poll};

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux stats: {e}");
        return 1;
    }
    let s = compute(&snap, now_unix());
    let json = std::env::args().any(|a| a == "--json")
        || std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] == "-o" && w[1] == "json");
    if json {
        print!("{}", render_json(&s));
    } else {
        print!("{}", render_text(&s, std::io::stdout().is_terminal()));
    }
    0
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// The computed report, kept as plain data so text and JSON rendering share it.
struct Stats {
    sessions: usize,
    windows: usize,
    panes: usize,
    clients: usize,
    attached_sessions: usize,
    dead_panes: usize,
    busiest_session: Option<(String, usize)>, // (name, pane count)
    largest_window: Option<(String, usize)>,  // (session:index name, pane count)
    oldest_session: Option<(String, i64)>,    // (name, age seconds)
    commands: Vec<(String, usize)>,           // per-command counts, desc
}

fn compute(snap: &Snapshot, now: i64) -> Stats {
    let mut panes_by_session: BTreeMap<&str, usize> = BTreeMap::new();
    let mut panes_by_window: BTreeMap<(&str, i64), usize> = BTreeMap::new();
    let mut commands: BTreeMap<&str, usize> = BTreeMap::new();
    let mut dead = 0usize;
    for p in &snap.panes {
        *panes_by_session.entry(p.session.as_str()).or_default() += 1;
        *panes_by_window
            .entry((p.session.as_str(), p.window))
            .or_default() += 1;
        if p.dead {
            dead += 1;
        }
        if !p.command.is_empty() {
            *commands.entry(p.command.as_str()).or_default() += 1;
        }
    }

    let busiest_session = panes_by_session
        .iter()
        .max_by_key(|(_, n)| **n)
        .map(|(name, n)| (name.to_string(), *n));

    let largest_window = panes_by_window
        .iter()
        .max_by_key(|(_, n)| **n)
        .map(|((sess, idx), n)| {
            let name = snap
                .windows
                .iter()
                .find(|w| w.session == *sess && w.index == *idx)
                .map_or_else(
                    || format!("{sess}:{idx}"),
                    |w| format!("{sess}:{idx} {}", w.name),
                );
            (name, *n)
        });

    let oldest_session = snap
        .sessions
        .iter()
        .filter(|s| s.created > 0)
        .min_by_key(|s| s.created)
        .map(|s| (s.name.clone(), (now - s.created).max(0)));

    let mut commands: Vec<(String, usize)> = commands
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    // Sort by count desc, then name for a stable order.
    commands.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    Stats {
        sessions: snap.sessions.len(),
        windows: snap.windows.len(),
        panes: snap.panes.len(),
        clients: snap.clients.len(),
        attached_sessions: snap.sessions.iter().filter(|s| s.attached).count(),
        dead_panes: dead,
        busiest_session,
        largest_window,
        oldest_session,
        commands,
    }
}

/// Format a duration in seconds as a compact human string (e.g. `3h 20m`).
fn human_age(secs: i64) -> String {
    let secs = secs.max(0);
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3_600;
    let m = (secs % 3_600) / 60;
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {}s", secs % 60)
    } else {
        format!("{secs}s")
    }
}

fn render_text(s: &Stats, color: bool) -> String {
    let paint = |t: &str, code: &str| -> String {
        if color {
            format!("\x1b[{code}m{t}\x1b[0m")
        } else {
            t.to_string()
        }
    };
    let mut out = String::new();
    out.push_str(&format!("{}\n\n", paint("ztmux stats", "1;36")));

    let row = |out: &mut String, k: &str, v: String| {
        out.push_str(&format!("  {k:<18} {v}\n"));
    };
    row(
        &mut out,
        "sessions",
        format!("{} ({} attached)", s.sessions, s.attached_sessions),
    );
    row(&mut out, "windows", s.windows.to_string());
    row(
        &mut out,
        "panes",
        if s.dead_panes > 0 {
            format!("{} ({} dead)", s.panes, s.dead_panes)
        } else {
            s.panes.to_string()
        },
    );
    row(&mut out, "clients", s.clients.to_string());
    if let Some((n, c)) = &s.busiest_session {
        row(&mut out, "busiest session", format!("{n} ({c} panes)"));
    }
    if let Some((n, c)) = &s.largest_window {
        row(&mut out, "largest window", format!("{n} ({c} panes)"));
    }
    if let Some((n, age)) = &s.oldest_session {
        row(
            &mut out,
            "oldest session",
            format!("{n} ({} old)", human_age(*age)),
        );
    }

    if !s.commands.is_empty() {
        out.push_str(&format!("\n{}\n", paint("commands", "1")));
        let max = s.commands.iter().map(|(_, n)| *n).max().unwrap_or(1).max(1);
        for (cmd, n) in s.commands.iter().take(12) {
            let bar_len = (*n * 20).div_ceil(max);
            let bar = "\u{2588}".repeat(bar_len);
            out.push_str(&format!("  {:<16} {} {}\n", cmd, paint(&bar, "36"), n));
        }
    }
    out
}

fn render_json(s: &Stats) -> String {
    let commands: Vec<serde_json::Value> = s
        .commands
        .iter()
        .map(|(c, n)| serde_json::json!({ "command": c, "count": n }))
        .collect();
    let v = serde_json::json!({
        "sessions": s.sessions,
        "attached_sessions": s.attached_sessions,
        "windows": s.windows,
        "panes": s.panes,
        "dead_panes": s.dead_panes,
        "clients": s.clients,
        "busiest_session": s.busiest_session.as_ref().map(|(n, c)|
            serde_json::json!({ "name": n, "panes": c })),
        "largest_window": s.largest_window.as_ref().map(|(n, c)|
            serde_json::json!({ "name": n, "panes": c })),
        "oldest_session": s.oldest_session.as_ref().map(|(n, age)|
            serde_json::json!({ "name": n, "age_seconds": age })),
        "commands": commands,
    });
    format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::super::tmux_query::{Client, Pane, Session, Window};
    use super::*;

    fn snap() -> Snapshot {
        Snapshot {
            sessions: vec![
                Session {
                    name: "work".into(),
                    created: 1000,
                    attached: true,
                    ..Default::default()
                },
                Session {
                    name: "old".into(),
                    created: 100,
                    ..Default::default()
                },
            ],
            windows: vec![Window {
                session: "work".into(),
                index: 0,
                name: "editor".into(),
                ..Default::default()
            }],
            panes: vec![
                Pane {
                    session: "work".into(),
                    window: 0,
                    command: "nvim".into(),
                    ..Default::default()
                },
                Pane {
                    session: "work".into(),
                    window: 0,
                    command: "nvim".into(),
                    ..Default::default()
                },
                Pane {
                    session: "work".into(),
                    window: 0,
                    command: "zsh".into(),
                    dead: true,
                    ..Default::default()
                },
            ],
            clients: vec![Client::default()],
            error: None,
        }
    }

    #[test]
    fn counts_and_histogram() {
        let s = compute(&snap(), 5000);
        assert_eq!(s.sessions, 2);
        assert_eq!(s.panes, 3);
        assert_eq!(s.dead_panes, 1);
        assert_eq!(s.attached_sessions, 1);
        // nvim (2) ranks before zsh (1)
        assert_eq!(s.commands.first().unwrap(), &("nvim".to_string(), 2));
        assert_eq!(
            s.busiest_session.as_ref().unwrap(),
            &("work".to_string(), 3)
        );
        assert_eq!(s.oldest_session.as_ref().unwrap().0, "old");
    }

    #[test]
    fn json_is_well_formed() {
        let s = compute(&snap(), 5000);
        let out = render_json(&s);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["panes"], 3);
        assert_eq!(v["commands"][0]["command"], "nvim");
    }

    #[test]
    fn human_age_formats() {
        assert_eq!(human_age(90_000), "1d 1h");
        assert_eq!(human_age(3_700), "1h 1m");
        assert_eq!(human_age(45), "45s");
    }

    // The largest window is identified by session:index and carries its name;
    // all three panes in the snapshot live in work:0 "editor".
    #[test]
    fn largest_window_names_the_window() {
        let s = compute(&snap(), 5000);
        let (name, panes) = s.largest_window.as_ref().unwrap();
        assert_eq!(panes, &3);
        assert_eq!(name, "work:0 editor");
    }

    // Oldest session is the one with the smallest created time and its age is
    // now - created (clamped at 0). "old" (created 100) beats "work" (1000).
    #[test]
    fn oldest_session_age_is_now_minus_created() {
        let s = compute(&snap(), 5000);
        let (name, age) = s.oldest_session.as_ref().unwrap();
        assert_eq!(name, "old");
        assert_eq!(*age, 4900);
    }

    // Commands are ranked by count descending, then name for a stable tie order.
    #[test]
    fn commands_ranked_by_count_then_name() {
        let s = compute(&snap(), 5000);
        assert_eq!(
            s.commands,
            vec![("nvim".to_string(), 2), ("zsh".to_string(), 1)]
        );
    }

    // An empty server has zero counts and no superlatives (all None/empty).
    #[test]
    fn empty_snapshot_has_no_superlatives() {
        let s = compute(&Snapshot::default(), 5000);
        assert_eq!(s.sessions, 0);
        assert_eq!(s.panes, 0);
        assert!(s.busiest_session.is_none());
        assert!(s.largest_window.is_none());
        assert!(s.oldest_session.is_none());
        assert!(s.commands.is_empty());
    }

    // human_age minute branch and zero/negative clamping.
    #[test]
    fn human_age_minute_and_clamp() {
        assert_eq!(human_age(125), "2m 5s");
        assert_eq!(human_age(0), "0s");
        assert_eq!(human_age(-5), "0s");
    }

    // The text report carries the header, the session line, the busiest-session
    // line, and the command histogram section.
    #[test]
    fn text_report_has_summary_and_histogram() {
        let s = compute(&snap(), 5000);
        let out = render_text(&s, false);
        assert!(out.contains("ztmux stats"));
        assert!(out.contains("2 (1 attached)"));
        assert!(out.contains("work (3 panes)"));
        assert!(out.contains("commands"));
        assert!(out.contains("nvim"));
    }
}
