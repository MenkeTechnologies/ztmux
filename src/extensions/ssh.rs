//! `ztmux ssh` — which panes hold an SSH connection, and to which host.
//!
//! One `ps` call lists every process's argv; the `ssh` clients are picked out,
//! their destination parsed from the command line, and each is attributed to the
//! pane it runs under by walking the process tree (like [`super::ports`] does for
//! sockets). This answers "which pane am I remoted into, and where" across the
//! whole server — the map of every live remote session. It degrades quietly: if
//! `ps` is missing or no pane owns an `ssh`, the table is empty. Output is a
//! table (coloured on a TTY) or a JSON array with `-o json` / `--json`, sorted by
//! location then target.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::process::Command;

use super::proctree::{ancestor_in, basename};
use super::tmux_query::{Pane, poll};

/// One process with its full argv (unlike [`super::proctree::Proc`], which keeps
/// only the basename) — the argv is needed to parse the ssh destination.
struct ProcArgs {
    pid: i64,
    ppid: i64,
    argv: String,
}

/// One attributed SSH session: the pane it runs in and the host it targets.
struct Row {
    pane: String,
    loc: String,
    pid: i64,
    target: String,
}

/// `ssh` option letters that take a separate argument (from ssh(1)); used to
/// skip a flag's value when scanning for the positional destination.
const ARG_FLAGS: &[char] = &[
    'B', 'b', 'c', 'D', 'E', 'e', 'F', 'I', 'i', 'J', 'L', 'l', 'm', 'O', 'o', 'P', 'p', 'Q', 'R',
    'S', 'W', 'w',
];

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux ssh: {e}");
        return 1;
    }
    let procs = gather();
    let rows = attribute(&procs, &snap.panes);
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

/// The whole process table with full argv via one `ps` call. Empty on failure.
fn gather() -> Vec<ProcArgs> {
    let Ok(out) = Command::new("ps")
        .args(["-A", "-o", "pid=,ppid=,args="])
        .output()
    else {
        return Vec::new();
    };
    parse_ps(&String::from_utf8_lossy(&out.stdout))
}

/// Parse `pid ppid argv…` lines: the first two whitespace fields are the pids,
/// the trimmed remainder is the full command line (which may contain spaces).
fn parse_ps(text: &str) -> Vec<ProcArgs> {
    text.lines()
        .filter_map(|line| {
            let rest = line.trim_start();
            let (pid_s, rest) = rest.split_once(char::is_whitespace)?;
            let pid: i64 = pid_s.parse().ok()?;
            let (ppid_s, argv) = rest.trim_start().split_once(char::is_whitespace)?;
            let ppid: i64 = ppid_s.parse().ok()?;
            let argv = argv.trim().to_string();
            if argv.is_empty() {
                return None;
            }
            Some(ProcArgs { pid, ppid, argv })
        })
        .collect()
}

/// True when a command line invokes the `ssh` client (argv[0]'s basename is
/// exactly `ssh`, so `sshd`, `ssh-agent`, `autossh` are excluded).
fn is_ssh(argv: &str) -> bool {
    argv.split_whitespace()
        .next()
        .is_some_and(|a0| basename(a0) == "ssh")
}

/// Parse the destination (`[user@]host`, an alias, or a `ssh://…` URI) out of an
/// `ssh` command line: the first non-option token, skipping option flags and the
/// values of the flags that take one. `None` if no destination is present.
fn parse_dest(argv: &str) -> Option<String> {
    let mut toks = argv.split_whitespace();
    toks.next()?; // argv[0] == ssh
    let mut skip_value = false;
    for tok in toks {
        if skip_value {
            skip_value = false;
            continue;
        }
        if let Some(flags) = tok.strip_prefix('-') {
            // A lone "-" is not a valid ssh flag; ignore it. Otherwise, if the
            // token ends in an argument-taking letter with no value attached
            // (e.g. "-p", "-ti"), the next token is that flag's value.
            if flags.chars().last().is_some_and(|c| ARG_FLAGS.contains(&c)) {
                skip_value = true;
            }
            continue;
        }
        return Some(tok.to_string());
    }
    None
}

/// Attribute each `ssh` process to the pane whose pid is the process's pid or an
/// ancestor of it. SSH processes under no pane are dropped. Sorted by location
/// then target.
fn attribute(procs: &[ProcArgs], panes: &[Pane]) -> Vec<Row> {
    let pane_pids: Vec<i64> = panes.iter().map(|p| p.pid).collect();
    let pm: HashMap<i64, i64> = procs.iter().map(|p| (p.pid, p.ppid)).collect();

    let mut rows: Vec<Row> = procs
        .iter()
        .filter(|p| is_ssh(&p.argv))
        .filter_map(|p| {
            let target = parse_dest(&p.argv)?;
            let owner = if pane_pids.contains(&p.pid) {
                Some(p.pid)
            } else {
                ancestor_in(p.pid, &pm, &pane_pids)
            };
            let owner = owner?;
            let pane = panes.iter().find(|pane| pane.pid == owner)?;
            Some(Row {
                pane: pane.id.clone(),
                loc: format!("{}:{}.{}", pane.session, pane.window, pane.index),
                pid: p.pid,
                target,
            })
        })
        .collect();
    rows.sort_by(|a, b| a.loc.cmp(&b.loc).then(a.target.cmp(&b.target)));
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
            &format!("{:<8} {:<16} {:>7} {}", "PANE", "LOCATION", "PID", "TARGET"),
            "1"
        )
    ));
    for r in rows {
        out.push_str(&format!(
            "{:<8} {:<16} {:>7} {}\n",
            r.pane, r.loc, r.pid, r.target,
        ));
    }
    out
}

fn render_json(rows: &[Row]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "pane": r.pane,
                "location": r.loc,
                "pid": r.pid,
                "target": r.target,
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
    fn parse_ps_splits_pids_and_keeps_full_argv() {
        let t = parse_ps("  100    50 ssh -p 2222 user@host\n 200 100 zsh\n");
        assert_eq!(t.len(), 2);
        assert_eq!((t[0].pid, t[0].ppid), (100, 50));
        assert_eq!(t[0].argv, "ssh -p 2222 user@host");
        assert_eq!(t[1].argv, "zsh");
    }

    #[test]
    fn is_ssh_matches_only_the_client() {
        assert!(is_ssh("ssh host"));
        assert!(is_ssh("/usr/bin/ssh host"));
        assert!(!is_ssh("sshd: user"));
        assert!(!is_ssh("ssh-agent"));
        assert!(!is_ssh("autossh -M 0 host"));
    }

    #[test]
    fn parse_dest_finds_bare_host() {
        assert_eq!(parse_dest("ssh myhost").as_deref(), Some("myhost"));
        assert_eq!(parse_dest("ssh user@box").as_deref(), Some("user@box"));
    }

    #[test]
    fn parse_dest_skips_flag_values() {
        // -p and -i take values; -tt is boolean and must not swallow the host.
        assert_eq!(
            parse_dest("ssh -p 2222 -i ~/.ssh/id -tt user@host echo hi").as_deref(),
            Some("user@host"),
        );
    }

    #[test]
    fn parse_dest_handles_attached_values() {
        // -p22 carries its value; the host is still found.
        assert_eq!(parse_dest("ssh -p22 host").as_deref(), Some("host"));
        // -o with a separate value is skipped.
        assert_eq!(
            parse_dest("ssh -o SendEnv=LANG host").as_deref(),
            Some("host")
        );
    }

    #[test]
    fn parse_dest_none_when_only_flags() {
        assert_eq!(parse_dest("ssh -V"), None);
        assert_eq!(parse_dest("ssh"), None);
    }

    fn panes() -> Vec<Pane> {
        vec![Pane {
            id: "%0".into(),
            session: "srv".into(),
            window: 2,
            index: 1,
            pid: 100,
            ..Default::default()
        }]
    }

    #[test]
    fn attributes_ssh_child_to_its_pane() {
        // ssh (300) → shell (100, the pane). Attributed up the tree.
        let procs = vec![
            ProcArgs {
                pid: 100,
                ppid: 1,
                argv: "zsh".into(),
            },
            ProcArgs {
                pid: 300,
                ppid: 100,
                argv: "ssh admin@prod".into(),
            },
        ];
        let rows = attribute(&procs, &panes());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pane, "%0");
        assert_eq!(rows[0].loc, "srv:2.1");
        assert_eq!(rows[0].target, "admin@prod");
        assert_eq!(rows[0].pid, 300);
    }

    #[test]
    fn ssh_under_no_pane_is_dropped() {
        let procs = vec![ProcArgs {
            pid: 999,
            ppid: 1,
            argv: "ssh lonely@host".into(),
        }];
        assert!(attribute(&procs, &panes()).is_empty());
    }

    #[test]
    fn non_ssh_processes_do_not_produce_rows() {
        let procs = vec![ProcArgs {
            pid: 100,
            ppid: 1,
            argv: "zsh".into(),
        }];
        assert!(attribute(&procs, &panes()).is_empty());
    }

    #[test]
    fn rows_sorted_by_location() {
        let panes = vec![
            Pane {
                id: "%1".into(),
                session: "a".into(),
                window: 0,
                index: 0,
                pid: 10,
                ..Default::default()
            },
            Pane {
                id: "%2".into(),
                session: "a".into(),
                window: 1,
                index: 0,
                pid: 20,
                ..Default::default()
            },
        ];
        let procs = vec![
            ProcArgs {
                pid: 20,
                ppid: 1,
                argv: "ssh b".into(),
            },
            ProcArgs {
                pid: 10,
                ppid: 1,
                argv: "ssh a".into(),
            },
        ];
        let rows = attribute(&procs, &panes);
        assert_eq!(
            rows.iter().map(|r| r.loc.as_str()).collect::<Vec<_>>(),
            vec!["a:0.0", "a:1.0"]
        );
    }
}
