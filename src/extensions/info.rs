//! `ztmux info` — a deep inspector for a single pane.
//!
//! Resolves a target (pane id `%N`, a `session:window.pane` location, or a
//! session name) to one pane and prints everything the query layer knows about
//! it in one place: identity and geometry, its process id/command/cpu/mem/rss
//! (via [`super::procstat`]), and the tail of its current screen (via
//! [`super::tmux_query::capture_pane`]). With `-o json` / `--json` the same is
//! emitted as one object. It is the one-pane complement to the server-wide
//! [`super::ps`] and [`super::peek`].

use std::io::IsTerminal;

use super::procstat::{ProcStat, fmt_rss, gather};
use super::tmux_query::{Pane, Snapshot, capture_pane, poll};

/// How many trailing screen lines to include.
const TAIL: usize = 10;

pub(crate) fn run(socket: &str) -> i32 {
    let Some(target) = target_arg() else {
        eprintln!("usage: ztmux info <target> [-o json]");
        return 2;
    };
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux info: {e}");
        return 1;
    }
    let Some(pane) = resolve(&snap, &target) else {
        eprintln!("ztmux info: no pane matched {target:?}");
        return 1;
    };
    let stat = gather(&[pane.pid]).get(&pane.pid).cloned().unwrap_or_default();
    let tail = capture_pane(socket, &pane.id, false)
        .map(|c| tail_lines(&c, TAIL))
        .unwrap_or_default();
    let json = std::env::args().any(|a| a == "--json")
        || std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] == "-o" && w[1] == "json");
    if json {
        print!("{}", render_json(pane, &stat, &tail));
    } else {
        print!(
            "{}",
            render_text(pane, &stat, &tail, std::io::stdout().is_terminal())
        );
    }
    0
}

/// The first positional argument after the `info` subcommand.
fn target_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let start = args.iter().position(|a| a == "info")? + 1;
    args[start..]
        .iter()
        .find(|a| !a.starts_with('-') && a.as_str() != "json")
        .cloned()
}

/// Resolve `target` to a pane, trying, in order: exact pane id (`%N`), exact
/// `session:window.pane` location, location prefix (`work:0` → first pane of
/// that window), then the first pane of a session named `target`.
fn resolve<'a>(snap: &'a Snapshot, target: &str) -> Option<&'a Pane> {
    let loc = |p: &Pane| format!("{}:{}.{}", p.session, p.window, p.index);
    if let Some(p) = snap.panes.iter().find(|p| p.id == target) {
        return Some(p);
    }
    if let Some(p) = snap.panes.iter().find(|p| loc(p) == target) {
        return Some(p);
    }
    let mut by_prefix: Vec<&Pane> = snap
        .panes
        .iter()
        .filter(|p| loc(p).starts_with(target))
        .collect();
    by_prefix.sort_by_key(|p| loc(p));
    if let Some(p) = by_prefix.first() {
        return Some(p);
    }
    let mut in_session: Vec<&Pane> = snap.panes.iter().filter(|p| p.session == target).collect();
    in_session.sort_by_key(|p| (p.window, p.index));
    in_session.first().copied()
}

/// The last `n` non-empty-trailing lines of a capture.
fn tail_lines(content: &str, n: usize) -> Vec<String> {
    let mut lines: Vec<String> = content.trim_end_matches('\n').lines().map(str::to_string).collect();
    let start = lines.len().saturating_sub(n);
    lines.split_off(start)
}

fn render_text(pane: &Pane, stat: &ProcStat, tail: &[String], color: bool) -> String {
    let paint = |s: &str, code: &str| -> String {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let loc = format!("{}:{}.{}", pane.session, pane.window, pane.index);
    let mut out = String::new();
    out.push_str(&format!("{}\n", paint(&format!("{} {}", pane.id, loc), "1;36")));
    let row = |k: &str, v: String| format!("  {:<10} {}\n", format!("{k}:"), v);
    out.push_str(&row("command", pane.command.clone()));
    out.push_str(&row("pid", pane.pid.to_string()));
    out.push_str(&row("title", pane.title.clone()));
    out.push_str(&row("path", pane.path.clone()));
    out.push_str(&row("size", format!("{}x{}", pane.width, pane.height)));
    out.push_str(&row(
        "flags",
        format!(
            "{}{}",
            if pane.active { "active " } else { "" },
            if pane.dead { "dead" } else { "live" }
        ),
    ));
    out.push_str(&row(
        "cpu/mem",
        format!("{:.1}% / {:.1}%", stat.cpu, stat.mem),
    ));
    out.push_str(&row("rss", fmt_rss(stat.rss_kb)));
    out.push_str(&row("state", stat.state.clone()));
    out.push_str(&format!("{}\n", paint("  screen:", "1")));
    for line in tail {
        out.push_str(&format!("  {line}\n"));
    }
    out
}

fn render_json(pane: &Pane, stat: &ProcStat, tail: &[String]) -> String {
    let loc = format!("{}:{}.{}", pane.session, pane.window, pane.index);
    let doc = serde_json::json!({
        "pane": pane.id,
        "location": loc,
        "session": pane.session,
        "command": pane.command,
        "pid": pane.pid,
        "title": pane.title,
        "path": pane.path,
        "width": pane.width,
        "height": pane.height,
        "active": pane.active,
        "dead": pane.dead,
        "cpu": stat.cpu,
        "mem": stat.mem,
        "rss_kb": stat.rss_kb,
        "state": stat.state,
        "screen_tail": tail,
    });
    format!("{}\n", serde_json::to_string_pretty(&doc).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap() -> Snapshot {
        Snapshot {
            panes: vec![
                Pane { id: "%0".into(), session: "work".into(), window: 0, index: 0, pid: 10, command: "zsh".into(), ..Default::default() },
                Pane { id: "%1".into(), session: "work".into(), window: 0, index: 1, pid: 11, command: "nvim".into(), ..Default::default() },
                Pane { id: "%2".into(), session: "ops".into(), window: 2, index: 0, pid: 12, command: "top".into(), ..Default::default() },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn resolves_by_pane_id() {
        let s = snap();
        assert_eq!(resolve(&s, "%1").unwrap().command, "nvim");
    }

    #[test]
    fn resolves_by_exact_location() {
        let s = snap();
        assert_eq!(resolve(&s, "work:0.1").unwrap().id, "%1");
    }

    #[test]
    fn resolves_by_window_prefix_to_first_pane() {
        // "work:0" matches both panes of window 0; the lowest location wins.
        let s = snap();
        assert_eq!(resolve(&s, "work:0").unwrap().id, "%0");
    }

    #[test]
    fn resolves_by_session_name_to_first_pane() {
        let s = snap();
        assert_eq!(resolve(&s, "ops").unwrap().id, "%2");
    }

    #[test]
    fn unknown_target_resolves_to_none() {
        let s = snap();
        assert!(resolve(&s, "nope").is_none());
    }

    #[test]
    fn tail_lines_takes_last_n_and_trims_trailing_blanks() {
        let content = "a\nb\nc\nd\n\n\n";
        let t = tail_lines(content, 2);
        assert_eq!(t, vec!["c".to_string(), "d".to_string()]);
    }

    #[test]
    fn json_carries_identity_stats_and_tail() {
        let pane = &snap().panes[1];
        let stat = ProcStat { cpu: 3.0, mem: 1.0, rss_kb: 2048, state: "S".into() };
        let tail = vec!["last line".to_string()];
        let v: serde_json::Value = serde_json::from_str(&render_json(pane, &stat, &tail)).unwrap();
        assert_eq!(v["pane"], "%1");
        assert_eq!(v["location"], "work:0.1");
        assert_eq!(v["pid"], 11);
        assert_eq!(v["cpu"], 3.0);
        assert_eq!(v["rss_kb"], 2048);
        assert_eq!(v["screen_tail"][0], "last line");
    }

    #[test]
    fn text_shows_fields_and_screen_section() {
        let pane = &snap().panes[0];
        let stat = ProcStat { cpu: 5.5, mem: 2.0, rss_kb: 1024, state: "R".into() };
        let s = render_text(pane, &stat, &["hello".to_string()], false);
        assert!(s.contains("%0 work:0.0"));
        assert!(s.contains("command:") && s.contains("zsh"));
        assert!(s.contains("cpu/mem:") && s.contains("5.5%"));
        assert!(s.contains("rss:") && s.contains("1.0M"));
        assert!(s.contains("screen:") && s.contains("hello"));
    }
}
