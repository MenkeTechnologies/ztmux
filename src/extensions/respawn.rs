//! `ztmux revive` — bring every dead pane back to life in place.
//!
//! With `remain-on-exit` set, a pane whose command exits stays open but dead,
//! showing its last output. Where [`super::prune`] *removes* those dead panes,
//! `respawn` *restarts* them — running `respawn-pane` on each so the whole server
//! is revived in one command instead of hunting them down. Optionally limited to
//! one session with `-s`. Like `prune` and `bcast`, it is **dry-run by default**
//! — it prints the dead panes it would revive — and only acts when `-f` /
//! `--force` is given.

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
        eprintln!("ztmux respawn: {e}");
        return 1;
    }
    let targets = select(&snap, args.session.as_deref());
    if targets.is_empty() {
        eprintln!("ztmux respawn: no dead panes");
        return 1;
    }
    if !args.force {
        println!("would respawn {} dead pane(s):", targets.len());
        for (id, loc, cmd) in &targets {
            println!("  {id} {loc} ({cmd})");
        }
        println!("(dry-run; pass -f to respawn)");
        return 0;
    }
    let mut done = 0;
    for (id, _, _) in &targets {
        if ztmux_cmd(socket, &["respawn-pane", "-t", id])
            .status()
            .is_ok_and(|s| s.success())
        {
            done += 1;
        }
    }
    println!("respawned {done}/{} pane(s)", targets.len());
    i32::from(done != targets.len())
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().collect();
    let rest = argv
        .iter()
        .position(|a| a == "respawn")
        .map_or(&[][..], |i| &argv[i + 1..]);
    Args {
        session: rest.windows(2).find(|w| w[0] == "-s").map(|w| w[1].clone()),
        force: rest.iter().any(|a| a == "-f" || a == "--force"),
    }
}

/// The dead panes (optionally within one session), as `(id, location, command)`,
/// ordered by location.
fn select(snap: &Snapshot, session: Option<&str>) -> Vec<(String, String, String)> {
    let mut sel: Vec<&Pane> = snap
        .panes
        .iter()
        .filter(|p| p.dead)
        .filter(|p| session.is_none_or(|s| p.session == s))
        .collect();
    sel.sort_by_key(|p| loc(p));
    sel.into_iter()
        .map(|p| (p.id.clone(), loc(p), p.command.clone()))
        .collect()
}

fn loc(p: &Pane) -> String {
    format!("{}:{}.{}", p.session, p.window, p.index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: &str, sess: &str, win: i64, idx: i64, dead: bool) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: "zsh".into(),
            dead,
            ..Default::default()
        }
    }

    fn snap() -> Snapshot {
        Snapshot {
            panes: vec![
                pane("%0", "work", 0, 0, false), // live → excluded
                pane("%1", "work", 1, 0, true),  // dead → included
                pane("%2", "ops", 0, 0, true),   // dead → included
            ],
            ..Default::default()
        }
    }

    #[test]
    fn only_dead_panes_are_selected() {
        let sel = select(&snap(), None);
        let ids: Vec<&str> = sel.iter().map(|(id, _, _)| id.as_str()).collect();
        // %2 (ops) sorts before %1 (work) by location; live %0 excluded.
        assert_eq!(ids, vec!["%2", "%1"]);
    }

    #[test]
    fn session_filter_restricts_to_one_session() {
        let sel = select(&snap(), Some("work"));
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].0, "%1");
        assert_eq!(sel[0].1, "work:1.0");
    }

    #[test]
    fn no_dead_panes_yields_empty() {
        let s = Snapshot {
            panes: vec![pane("%0", "a", 0, 0, false)],
            ..Default::default()
        };
        assert!(select(&s, None).is_empty());
    }

    #[test]
    fn selection_carries_location_and_command() {
        let sel = select(&snap(), Some("ops"));
        assert_eq!(sel[0], ("%2".into(), "ops:0.0".into(), "zsh".into()));
    }
}
