//! `ztmux equalize` — reset every multi-pane window to a balanced layout.
//!
//! After a session of ad-hoc splits, windows drift into lopsided layouts. Where
//! [`super::layout`] applies a preset to one window, `equalize` sweeps the whole
//! server (or one session with `-s`) and re-lays every window that has more than
//! one pane, so a single command re-balances everything. The target layout is
//! `tiled` by default, or any of the five named tmux layouts as a positional
//! argument. Like `prune`, `bcast`, and `layout`, it is **dry-run by default** —
//! it prints the windows it would touch — and only applies when `-f` / `--force`
//! is given.

use super::tmux_query::{Snapshot, Window, poll, ztmux_cmd};

/// The five named tmux layouts `select-layout` accepts.
const LAYOUTS: &[&str] = &[
    "even-horizontal",
    "even-vertical",
    "main-horizontal",
    "main-vertical",
    "tiled",
];

/// Parsed invocation: the target layout and optional session filter.
struct Args {
    layout: String,
    session: Option<String>,
    force: bool,
}

/// One window that would be re-laid: its `session:index` target, a display
/// location, and its pane count.
struct Target {
    target: String,
    location: String,
    panes: i64,
}

pub(crate) fn run(socket: &str) -> i32 {
    let Some(args) = parse_args() else {
        eprintln!(
            "usage: ztmux equalize [layout] [-s session] [-f]\n  layout: {}",
            LAYOUTS.join(" | ")
        );
        return 2;
    };
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux equalize: {e}");
        return 1;
    }
    let targets = select(&snap, args.session.as_deref());
    if targets.is_empty() {
        eprintln!("ztmux equalize: no multi-pane windows matched");
        return 1;
    }
    if !args.force {
        println!(
            "would set layout {:?} on {} window(s):",
            args.layout,
            targets.len()
        );
        for t in &targets {
            println!("  {} ({} panes)", t.location, t.panes);
        }
        println!("(dry-run; pass -f to apply)");
        return 0;
    }
    let mut done = 0;
    for t in &targets {
        if ztmux_cmd(socket, &["select-layout", "-t", &t.target, &args.layout])
            .status()
            .is_ok_and(|s| s.success())
        {
            done += 1;
        }
    }
    println!(
        "re-laid {done}/{} window(s) as {}",
        targets.len(),
        args.layout
    );
    i32::from(done != targets.len())
}

/// Parse the optional layout positional and `-s` / `-f` flags. Returns `None` on
/// an unknown layout name (usage error).
fn parse_args() -> Option<Args> {
    let argv: Vec<String> = std::env::args().collect();
    let start = argv.iter().position(|a| a == "equalize")? + 1;
    let rest = &argv[start..];

    let session = rest.windows(2).find(|w| w[0] == "-s").map(|w| w[1].clone());
    let force = rest.iter().any(|a| a == "-f" || a == "--force");

    // The layout is the first bare positional that is neither a flag nor the
    // value consumed by -s. Default to "tiled" when absent.
    let mut layout = None;
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == "-s" {
            i += 2; // skip the flag and its value
            continue;
        }
        if !rest[i].starts_with('-') {
            layout = Some(rest[i].clone());
            break;
        }
        i += 1;
    }
    let layout = layout.unwrap_or_else(|| "tiled".to_string());
    if !LAYOUTS.contains(&layout.as_str()) {
        return None;
    }
    Some(Args {
        layout,
        session,
        force,
    })
}

/// Windows with more than one pane (optionally within one session), as targets.
/// Single-pane windows are skipped — there is nothing to balance.
fn select(snap: &Snapshot, session: Option<&str>) -> Vec<Target> {
    let mut sel: Vec<&Window> = snap
        .windows
        .iter()
        .filter(|w| w.panes > 1)
        .filter(|w| session.is_none_or(|s| w.session == s))
        .collect();
    sel.sort_by(|a, b| a.session.cmp(&b.session).then(a.index.cmp(&b.index)));
    sel.into_iter()
        .map(|w| Target {
            target: format!("{}:{}", w.session, w.index),
            location: format!("{}:{} ({})", w.session, w.index, w.name),
            panes: w.panes,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(session: &str, index: i64, name: &str, panes: i64) -> Window {
        Window {
            session: session.into(),
            index,
            name: name.into(),
            panes,
            ..Default::default()
        }
    }

    fn snap() -> Snapshot {
        Snapshot {
            windows: vec![
                win("work", 0, "edit", 3),
                win("work", 1, "solo", 1), // single pane → skipped
                win("ops", 0, "logs", 2),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn single_pane_windows_are_skipped() {
        let t = select(&snap(), None);
        let locs: Vec<&str> = t.iter().map(|t| t.target.as_str()).collect();
        assert_eq!(locs, vec!["ops:0", "work:0"]); // sorted by session; work:1 excluded
    }

    #[test]
    fn session_filter_restricts_targets() {
        let t = select(&snap(), Some("work"));
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].target, "work:0");
        assert_eq!(t[0].panes, 3);
    }

    #[test]
    fn no_multipane_windows_yields_empty() {
        let s = Snapshot {
            windows: vec![win("a", 0, "w", 1)],
            ..Default::default()
        };
        assert!(select(&s, None).is_empty());
    }

    #[test]
    fn targets_are_session_colon_index() {
        let t = select(&snap(), Some("ops"));
        assert_eq!(t[0].target, "ops:0");
        assert!(t[0].location.contains("logs"));
    }
}
