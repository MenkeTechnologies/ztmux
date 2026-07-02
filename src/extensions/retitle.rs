//! `ztmux retitle` — label every pane with the command it is running.
//!
//! The write-side companion to [`super::titles`]: it sets each live pane's title
//! (via `select-pane -T`) to the command currently running in it, so a status
//! line or `ztmux titles` shows `nvim` / `psql` / `ssh` instead of a stale or
//! default title. Only panes whose title already differs from their command are
//! touched (so re-running it is a no-op), optionally within one session with
//! `-s`. Like `prune`, it is **dry-run by default** — it prints the retitles it
//! would make — and only applies when `-f` / `--force` is given.

use super::tmux_query::{Pane, Snapshot, poll, ztmux_cmd};

/// Parsed invocation: optional session filter and the force flag.
struct Args {
    session: Option<String>,
    force: bool,
}

/// One pane that would be retitled: its id, location, current title, and the
/// command that will become its new title.
struct Target {
    id: String,
    location: String,
    old: String,
    command: String,
}

pub(crate) fn run(socket: &str) -> i32 {
    let args = parse_args();
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux retitle: {e}");
        return 1;
    }
    let targets = select(&snap, args.session.as_deref());
    if targets.is_empty() {
        eprintln!("ztmux retitle: every pane already titled by its command");
        return 1;
    }
    if !args.force {
        println!("would retitle {} pane(s):", targets.len());
        for t in &targets {
            println!("  {} {} {:?} -> {:?}", t.id, t.location, t.old, t.command);
        }
        println!("(dry-run; pass -f to apply)");
        return 0;
    }
    let mut done = 0;
    for t in &targets {
        if ztmux_cmd(socket, &["select-pane", "-t", &t.id, "-T", &t.command])
            .status()
            .is_ok_and(|s| s.success())
        {
            done += 1;
        }
    }
    println!("retitled {done}/{} pane(s)", targets.len());
    i32::from(done != targets.len())
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().collect();
    let rest = argv
        .iter()
        .position(|a| a == "retitle")
        .map_or(&[][..], |i| &argv[i + 1..]);
    Args {
        session: rest.windows(2).find(|w| w[0] == "-s").map(|w| w[1].clone()),
        force: rest.iter().any(|a| a == "-f" || a == "--force"),
    }
}

/// Live panes whose title differs from their running command (optionally within
/// one session), ordered by location. Panes already titled by their command are
/// skipped so applying is idempotent.
fn select(snap: &Snapshot, session: Option<&str>) -> Vec<Target> {
    let mut sel: Vec<&Pane> = snap
        .panes
        .iter()
        .filter(|p| !p.dead && !p.command.is_empty() && p.title != p.command)
        .filter(|p| session.is_none_or(|s| p.session == s))
        .collect();
    sel.sort_by_key(|p| loc(p));
    sel.into_iter()
        .map(|p| Target {
            id: p.id.clone(),
            location: loc(p),
            old: p.title.clone(),
            command: p.command.clone(),
        })
        .collect()
}

fn loc(p: &Pane) -> String {
    format!("{}:{}.{}", p.session, p.window, p.index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: &str, sess: &str, win: i64, idx: i64, cmd: &str, title: &str) -> Pane {
        Pane {
            session: sess.into(),
            window: win,
            index: idx,
            id: id.into(),
            command: cmd.into(),
            title: title.into(),
            ..Default::default()
        }
    }

    fn snap() -> Snapshot {
        Snapshot {
            panes: vec![
                pane("%0", "work", 0, 0, "nvim", "nvim"), // already matches → skipped
                pane("%1", "work", 1, 0, "psql", "host-title"), // differs → included
                pane("%2", "ops", 0, 0, "ssh", ""),       // empty title differs → included
            ],
            ..Default::default()
        }
    }

    #[test]
    fn only_mismatched_titles_are_targeted() {
        let sel = select(&snap(), None);
        let ids: Vec<&str> = sel.iter().map(|t| t.id.as_str()).collect();
        // %2 (ops) sorts first by location; %0 already-matching is excluded.
        assert_eq!(ids, vec!["%2", "%1"]);
    }

    #[test]
    fn new_title_is_the_command() {
        let sel = select(&snap(), Some("work"));
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].id, "%1");
        assert_eq!(sel[0].old, "host-title");
        assert_eq!(sel[0].command, "psql");
    }

    #[test]
    fn all_matching_yields_empty() {
        let s = Snapshot {
            panes: vec![pane("%0", "a", 0, 0, "zsh", "zsh")],
            ..Default::default()
        };
        assert!(select(&s, None).is_empty());
    }

    #[test]
    fn commandless_panes_are_skipped() {
        let s = Snapshot {
            panes: vec![pane("%0", "a", 0, 0, "", "something")],
            ..Default::default()
        };
        assert!(select(&s, None).is_empty());
    }
}
