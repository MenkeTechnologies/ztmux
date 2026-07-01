//! `ztmux prune` — find and remove dead/empty/idle server objects.
//!
//! A client subcommand over the `list-* -o json` query layer that identifies
//! cleanup candidates — dead panes, sessions with no windows, and (optionally)
//! detached sessions idle beyond a threshold — and kills them via the ztmux
//! CLI. It is **dry-run by default**: it prints what it *would* remove and
//! exits; pass `-f`/`--force` to actually kill. Server-native housekeeping no
//! GUI client can do on its own.
//!
//! Flags: `--dead` dead panes · `--empty` window-less sessions ·
//! `--idle <secs>` detached sessions idle longer than N seconds ·
//! `-f`/`--force` execute (otherwise dry-run) · `-o json`.
//! With no selector flag, `--dead` and `--empty` are both enabled.

use std::io::IsTerminal;
use std::time::{SystemTime, UNIX_EPOCH};

use super::tmux_query::{Snapshot, poll, ztmux_cmd};

/// A single prune candidate.
struct Victim {
    kind: &'static str, // "pane" | "session"
    cmd: &'static str,  // kill-pane | kill-session
    target: String,
    reason: String,
}

struct Opts {
    dead: bool,
    empty: bool,
    idle: Option<i64>,
    force: bool,
    json: bool,
}

fn parse_opts() -> Opts {
    let args: Vec<String> = std::env::args().collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let idle = args
        .iter()
        .position(|a| a == "--idle")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok());
    let (dead, empty) = (has("--dead"), has("--empty"));
    // Default selection when no selector is given: dead panes + empty sessions.
    let none_selected = !dead && !empty && idle.is_none();
    Opts {
        dead: dead || none_selected,
        empty: empty || none_selected,
        idle,
        force: has("-f") || has("--force"),
        json: has("--json") || args.windows(2).any(|w| w[0] == "-o" && w[1] == "json"),
    }
}

pub(crate) fn run(socket: &str) -> i32 {
    let snap = poll(socket);
    if let Some(e) = &snap.error {
        eprintln!("ztmux prune: {e}");
        return 1;
    }
    let opts = parse_opts();
    let victims = plan(&snap, &opts, now_unix());

    if opts.json {
        print!("{}", render_json(&victims, opts.force));
    } else {
        print!(
            "{}",
            render_text(&victims, opts.force, std::io::stdout().is_terminal())
        );
    }

    if opts.force {
        let mut failed = 0;
        for v in &victims {
            let out = ztmux_cmd(socket, &[v.cmd, "-t", &v.target]).output();
            if !matches!(out, Ok(o) if o.status.success()) {
                failed += 1;
            }
        }
        return i32::from(failed > 0);
    }
    0
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Compute the prune candidates for a snapshot (pure — no killing).
fn plan(snap: &Snapshot, opts: &Opts, now: i64) -> Vec<Victim> {
    let mut out = Vec::new();

    if opts.dead {
        for p in snap.panes.iter().filter(|p| p.dead) {
            out.push(Victim {
                kind: "pane",
                cmd: "kill-pane",
                target: p.id.clone(),
                reason: "dead".into(),
            });
        }
    }

    if opts.empty {
        for s in snap.sessions.iter().filter(|s| s.windows == 0) {
            out.push(Victim {
                kind: "session",
                cmd: "kill-session",
                target: s.name.clone(),
                reason: "no windows".into(),
            });
        }
    }

    if let Some(threshold) = opts.idle {
        for s in &snap.sessions {
            if !s.attached && s.activity > 0 && now - s.activity > threshold {
                out.push(Victim {
                    kind: "session",
                    cmd: "kill-session",
                    target: s.name.clone(),
                    reason: format!("idle {}s", now - s.activity),
                });
            }
        }
    }
    out
}

fn render_text(victims: &[Victim], force: bool, color: bool) -> String {
    let paint = |s: &str, code: &str| -> String {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let mut out = String::new();
    let verb = if force { "pruning" } else { "would prune" };
    if victims.is_empty() {
        out.push_str("nothing to prune\n");
        return out;
    }
    out.push_str(&format!(
        "{} {} object(s):\n",
        paint("ztmux prune", "1;36"),
        victims.len()
    ));
    for v in victims {
        out.push_str(&format!(
            "  {} {} {} ({})\n",
            paint(verb, if force { "31" } else { "33" }),
            v.kind,
            v.target,
            v.reason
        ));
    }
    if !force {
        out.push_str(&format!(
            "\n{}\n",
            paint("re-run with -f/--force to remove", "2")
        ));
    }
    out
}

fn render_json(victims: &[Victim], force: bool) -> String {
    let arr: Vec<serde_json::Value> = victims
        .iter()
        .map(|v| serde_json::json!({ "kind": v.kind, "target": v.target, "reason": v.reason }))
        .collect();
    let v = serde_json::json!({ "force": force, "victims": arr });
    format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::super::tmux_query::{Pane, Session};
    use super::*;

    fn opts(dead: bool, empty: bool, idle: Option<i64>) -> Opts {
        Opts {
            dead,
            empty,
            idle,
            force: false,
            json: false,
        }
    }

    fn snap() -> Snapshot {
        Snapshot {
            sessions: vec![
                Session {
                    name: "empty".into(),
                    windows: 0,
                    ..Default::default()
                },
                Session {
                    name: "old".into(),
                    windows: 1,
                    attached: false,
                    activity: 100,
                    ..Default::default()
                },
                Session {
                    name: "live".into(),
                    windows: 1,
                    attached: true,
                    activity: 4900,
                    ..Default::default()
                },
            ],
            panes: vec![
                Pane {
                    id: "%0".into(),
                    dead: true,
                    ..Default::default()
                },
                Pane {
                    id: "%1".into(),
                    dead: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn dead_panes_and_empty_sessions() {
        let v = plan(&snap(), &opts(true, true, None), 5000);
        assert!(v.iter().any(|x| x.kind == "pane" && x.target == "%0"));
        assert!(v.iter().any(|x| x.kind == "session" && x.target == "empty"));
        // live pane and non-empty sessions are spared
        assert!(!v.iter().any(|x| x.target == "%1"));
        assert!(!v.iter().any(|x| x.target == "live"));
    }

    #[test]
    fn idle_threshold_targets_detached_stale_sessions() {
        // now=5000, threshold=1000: "old" (activity 100 -> idle 4900) qualifies;
        // "live" is attached so it is spared.
        let v = plan(&snap(), &opts(false, false, Some(1000)), 5000);
        assert!(v.iter().any(|x| x.target == "old"));
        assert!(!v.iter().any(|x| x.target == "live"));
    }

    #[test]
    fn dry_run_text_explains_how_to_force() {
        let v = plan(&snap(), &opts(true, true, None), 5000);
        let s = render_text(&v, false, false);
        assert!(s.contains("would prune"));
        assert!(s.contains("-f/--force"));
    }

    // JSON output lists each victim's target/reason and the force flag.
    #[test]
    fn json_lists_victims_and_force_flag() {
        let v = plan(&snap(), &opts(true, true, None), 5000);
        let out = render_json(&v, false);
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(j["force"], false);
        let arr = j["victims"].as_array().unwrap();
        assert!(
            arr.iter()
                .any(|x| x["target"] == "%0" && x["reason"] == "dead")
        );
        assert!(
            arr.iter()
                .any(|x| x["target"] == "empty" && x["reason"] == "no windows")
        );
    }

    // Raising the idle threshold above every session's idle time spares them
    // all (old is idle 4900; threshold 10000 > 4900).
    #[test]
    fn idle_spares_recently_active_sessions() {
        let v = plan(&snap(), &opts(false, false, Some(10_000)), 5000);
        assert!(v.iter().all(|x| x.target != "old"));
    }

    // With nothing to prune, the text report is exactly the one-line notice.
    #[test]
    fn empty_plan_reports_nothing() {
        let s = render_text(&[], false, false);
        assert_eq!(s, "nothing to prune\n");
    }

    // Under --force the verb is "pruning" and the dry-run hint is suppressed.
    #[test]
    fn force_text_uses_pruning_verb() {
        let v = plan(&snap(), &opts(true, false, None), 5000);
        let s = render_text(&v, true, false);
        assert!(s.contains("pruning"));
        assert!(!s.contains("-f/--force"));
    }
}
