//! `ztmux bcast` — broadcast a command to many panes at once.
//!
//! Sends the given command to every pane matching an optional filter, so a
//! single invocation drives a whole fleet of panes (e.g. `git pull` in every
//! pane running a shell). It is the cross-server generalisation of tmux's
//! window-local `synchronize-panes`.
//!
//! Filters (all optional, combined with logical and):
//!   `-c <substr>`  only panes whose command contains the substring
//!   `-s <session>` only panes in the named session
//!
//! By default the command line is sent literally followed by Enter (so it
//! runs); `-N` / `--no-enter` sends the keys without the trailing Enter. Like
//! `prune` and `layout`, it is **dry-run by default** — it prints the panes it
//! would target — and only sends when `-f` / `--force` is given.

use super::tmux_query::{Pane, Snapshot, poll, ztmux_cmd};

/// Parsed invocation: the command to send plus the selection filters.
struct Args {
    command: String,
    command_filter: Option<String>,
    session: Option<String>,
    enter: bool,
    force: bool,
}

pub(crate) fn run(socket: &str) -> i32 {
    let Some(args) = parse_args() else {
        eprintln!("usage: ztmux bcast <command> [-c cmd] [-s session] [-N] [-f]");
        return 2;
    };
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux bcast: {e}");
        return 1;
    }
    let targets = select(&snap, &args);
    if targets.is_empty() {
        eprintln!("ztmux bcast: no panes matched");
        return 1;
    }
    if !args.force {
        println!(
            "would send {:?}{} to {} pane(s):",
            args.command,
            if args.enter { " + Enter" } else { "" },
            targets.len()
        );
        for t in &targets {
            println!("  {} {}", t.0, t.1);
        }
        println!("(dry-run; pass -f to send)");
        return 0;
    }
    let mut sent = 0;
    for (id, _) in &targets {
        if send(socket, id, &args.command, args.enter) {
            sent += 1;
        }
    }
    println!("sent to {sent}/{} pane(s)", targets.len());
    i32::from(sent != targets.len())
}

/// Parse the command (first positional) and flags from the process args.
fn parse_args() -> Option<Args> {
    let argv: Vec<String> = std::env::args().collect();
    let start = argv.iter().position(|a| a == "bcast")? + 1;
    let rest = &argv[start..];

    let value_after = |flag: &str| -> Option<String> {
        rest.windows(2).find(|w| w[0] == flag).map(|w| w[1].clone())
    };
    let command_filter = value_after("-c");
    let session = value_after("-s");
    let enter = !rest.iter().any(|a| a == "-N" || a == "--no-enter");
    let force = rest.iter().any(|a| a == "-f" || a == "--force");

    // The command is the first bare positional that is not a flag or the value
    // consumed by -c / -s.
    let consumed: Vec<&String> = rest
        .iter()
        .enumerate()
        .filter_map(|(i, a)| (i > 0 && (rest[i - 1] == "-c" || rest[i - 1] == "-s")).then_some(a))
        .collect();
    let command = rest
        .iter()
        .find(|a| !a.starts_with('-') && !consumed.contains(a))?
        .clone();

    Some(Args {
        command,
        command_filter,
        session,
        enter,
        force,
    })
}

/// The panes matching every active filter, as `(pane-id, location)` pairs,
/// ordered by location.
fn select(snap: &Snapshot, args: &Args) -> Vec<(String, String)> {
    let mut sel: Vec<&Pane> = snap
        .panes
        .iter()
        .filter(|p| !p.dead)
        .filter(|p| {
            args.command_filter
                .as_ref()
                .is_none_or(|c| p.command.contains(c))
        })
        .filter(|p| args.session.as_ref().is_none_or(|s| &p.session == s))
        .collect();
    sel.sort_by_key(|p| loc(p));
    sel.into_iter().map(|p| (p.id.clone(), loc(p))).collect()
}

fn loc(p: &Pane) -> String {
    format!("{}:{}.{}", p.session, p.window, p.index)
}

/// Send `command` to one pane: literal keys, optionally followed by Enter.
/// Returns whether the send-keys call(s) succeeded.
fn send(socket: &str, id: &str, command: &str, enter: bool) -> bool {
    let ok = ztmux_cmd(socket, &["send-keys", "-t", id, "-l", "--", command])
        .status()
        .is_ok_and(|s| s.success());
    if ok && enter {
        return ztmux_cmd(socket, &["send-keys", "-t", id, "Enter"])
            .status()
            .is_ok_and(|s| s.success());
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap() -> Snapshot {
        Snapshot {
            panes: vec![
                Pane {
                    id: "%0".into(),
                    session: "work".into(),
                    window: 0,
                    index: 0,
                    command: "zsh".into(),
                    ..Default::default()
                },
                Pane {
                    id: "%1".into(),
                    session: "work".into(),
                    window: 0,
                    index: 1,
                    command: "nvim".into(),
                    ..Default::default()
                },
                Pane {
                    id: "%2".into(),
                    session: "ops".into(),
                    window: 0,
                    index: 0,
                    command: "zsh".into(),
                    ..Default::default()
                },
                Pane {
                    id: "%3".into(),
                    session: "ops".into(),
                    window: 1,
                    index: 0,
                    command: "zsh".into(),
                    dead: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    fn args(command_filter: Option<&str>, session: Option<&str>) -> Args {
        Args {
            command: "git pull".into(),
            command_filter: command_filter.map(Into::into),
            session: session.map(Into::into),
            enter: true,
            force: false,
        }
    }

    #[test]
    fn no_filter_selects_every_live_pane() {
        let sel = select(&snap(), &args(None, None));
        // The dead %3 is excluded; %0,%1,%2 remain.
        let ids: Vec<&str> = sel.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, vec!["%2", "%0", "%1"]); // sorted by location
    }

    #[test]
    fn command_filter_restricts_by_running_command() {
        let sel = select(&snap(), &args(Some("zsh"), None));
        let ids: Vec<&str> = sel.iter().map(|(id, _)| id.as_str()).collect();
        // Only live zsh panes: %0 (work) and %2 (ops); nvim %1 and dead %3 out.
        assert_eq!(ids, vec!["%2", "%0"]);
    }

    #[test]
    fn session_filter_restricts_by_session() {
        let sel = select(&snap(), &args(None, Some("work")));
        let ids: Vec<&str> = sel.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, vec!["%0", "%1"]);
    }

    #[test]
    fn filters_and_together() {
        let sel = select(&snap(), &args(Some("zsh"), Some("ops")));
        let ids: Vec<&str> = sel.iter().map(|(id, _)| id.as_str()).collect();
        // ops + zsh + live → only %2.
        assert_eq!(ids, vec!["%2"]);
    }

    #[test]
    fn dead_panes_are_never_targeted() {
        // Even a session/command matching the dead pane excludes it.
        let sel = select(&snap(), &args(Some("zsh"), Some("ops")));
        assert!(!sel.iter().any(|(id, _)| id == "%3"));
    }

    #[test]
    fn selection_carries_locations() {
        let sel = select(&snap(), &args(None, Some("ops")));
        assert_eq!(sel, vec![("%2".to_string(), "ops:0.0".to_string())]);
    }
}
