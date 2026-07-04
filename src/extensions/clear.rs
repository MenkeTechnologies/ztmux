//! `ztmux clearall` — free the scrollback buffer of every pane at once.
//!
//! The write-side companion to [`super::history`]: where `history` shows which
//! panes are hoarding scrollback, `clear` runs `clear-history` on every pane
//! that holds any (optionally within one session with `-s`), reclaiming that
//! memory in a single command. Panes with an empty buffer are skipped. Like
//! `prune`, it is **dry-run by default** — it prints the panes it would clear and
//! how many lines each is holding — and only acts when `-f` / `--force` is given.

use super::tmux_query::{Pane, Snapshot, poll, ztmux_cmd};

/// Parsed invocation: optional session filter and the force flag.
struct Args {
    session: Option<String>,
    force: bool,
}

pub(crate) fn run(socket: &str) -> i32 {
    let args = parse_args();
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux clear: {e}");
        return 1;
    }
    let targets = select(&snap, args.session.as_deref());
    if targets.is_empty() {
        eprintln!("ztmux clear: no panes with scrollback");
        return 1;
    }
    let total: i64 = targets.iter().map(|(_, _, n)| n).sum();
    if !args.force {
        println!(
            "would clear {} line(s) of scrollback across {} pane(s):",
            total,
            targets.len()
        );
        for (id, loc, lines) in &targets {
            println!("  {id} {loc} ({lines} lines)");
        }
        println!("(dry-run; pass -f to clear)");
        return 0;
    }
    let mut done = 0;
    for (id, _, _) in &targets {
        if ztmux_cmd(socket, &["clear-history", "-t", id])
            .status()
            .is_ok_and(|s| s.success())
        {
            done += 1;
        }
    }
    println!("cleared {done}/{} pane(s)", targets.len());
    i32::from(done != targets.len())
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().collect();
    let rest = argv
        .iter()
        .position(|a| a == "clear")
        .map_or(&[][..], |i| &argv[i + 1..]);
    Args {
        session: rest.windows(2).find(|w| w[0] == "-s").map(|w| w[1].clone()),
        force: rest.iter().any(|a| a == "-f" || a == "--force"),
    }
}

/// Live panes holding scrollback (optionally within one session), as
/// `(id, location, lines)`, ordered by location. Panes with an empty buffer are
/// skipped — clearing them is a no-op.
fn select(snap: &Snapshot, session: Option<&str>) -> Vec<(String, String, i64)> {
    let mut sel: Vec<&Pane> = snap
        .panes
        .iter()
        .filter(|p| !p.dead && p.history_size > 0)
        .filter(|p| session.is_none_or(|s| p.session == s))
        .collect();
    sel.sort_by_key(|p| loc(p));
    sel.into_iter()
        .map(|p| (p.id.clone(), loc(p), p.history_size))
        .collect()
}

fn loc(p: &Pane) -> String {
    format!("{}:{}.{}", p.session, p.window, p.index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: &str, sess: &str, win: i64, idx: i64, hist: i64) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: "zsh".into(),
            history_size: hist,
            ..Default::default()
        }
    }

    fn snap() -> Snapshot {
        Snapshot {
            panes: vec![
                pane("%0", "work", 0, 0, 0),   // empty buffer → skipped
                pane("%1", "work", 1, 0, 500), // has scrollback → included
                pane("%2", "ops", 0, 0, 9000), // has scrollback → included
            ],
            ..Default::default()
        }
    }

    #[test]
    fn only_panes_with_scrollback_are_selected() {
        let sel = select(&snap(), None);
        let ids: Vec<&str> = sel.iter().map(|(id, _, _)| id.as_str()).collect();
        // %2 (ops) sorts before %1 (work) by location; empty %0 excluded.
        assert_eq!(ids, vec!["%2", "%1"]);
    }

    #[test]
    fn session_filter_restricts_targets() {
        let sel = select(&snap(), Some("ops"));
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].0, "%2");
        assert_eq!(sel[0].2, 9000);
    }

    #[test]
    fn no_scrollback_anywhere_yields_empty() {
        let s = Snapshot {
            panes: vec![pane("%0", "a", 0, 0, 0)],
            ..Default::default()
        };
        assert!(select(&s, None).is_empty());
    }

    #[test]
    fn selection_carries_line_counts() {
        let sel = select(&snap(), Some("work"));
        assert_eq!(sel[0], ("%1".to_string(), "work:1.0".to_string(), 500));
    }
}
