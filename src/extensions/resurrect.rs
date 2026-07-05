//! `ztmux resurrect` — save and restore the whole server across restarts.
//!
//! Zellij persists sessions so they survive a restart/reboot; tmux does not.
//! `resurrect save` walks every session → window → pane and writes its shape —
//! window names and layouts, and each pane's working directory and running
//! command — to `~/.ztmux/resurrect/`. `resurrect restore` reads it back and
//! recreates the sessions: windows in order, the right number of panes (each in
//! its saved cwd), and the exact tiled geometry via the saved layout string.
//!
//! A shell pane restores perfectly (same cwd). An arbitrary running program
//! cannot be resumed, so by default panes come back as a shell in the right
//! directory; `--run` additionally re-sends each pane's saved command. Existing
//! sessions are never clobbered — a session whose name is already present is
//! skipped.
//!
//! Subcommands: `save` (default), `restore [file] [--run]`, `list`.

use std::path::PathBuf;

use super::tmux_query::query_lines;

/// Field separator in the save file (unit separator: never appears in names,
/// paths or layout strings).
const SEP: char = '\u{1f}';

struct Pane {
    active: bool,
    cwd: String,
    command: String,
}

struct Win {
    session: String,
    index: String,
    name: String,
    layout: String,
    active: bool,
    panes: Vec<Pane>,
}

pub(crate) fn run(socket: &str) -> i32 {
    match op_arg().as_deref() {
        Some("restore") => restore(socket),
        Some("list") => list(),
        Some("autosave") => autosave(socket),
        _ => save(socket),
    }
}

/// The subcommand word after `resurrect`.
fn op_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == "resurrect")?;
    args.get(i + 1).filter(|s| !s.starts_with('-')).cloned()
}

/// `~/.ztmux/resurrect`, created if missing.
fn dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let d = PathBuf::from(home).join(".ztmux").join("resurrect");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

// ---- save ----------------------------------------------------------------

/// Capture the whole server as the save-file text (window + pane lines).
fn capture(socket: &str) -> String {
    let wfmt = format!(
        "win{SEP}#{{session_name}}{SEP}#{{window_index}}{SEP}#{{window_name}}{SEP}#{{window_layout}}{SEP}#{{window_active}}"
    );
    let pfmt = format!(
        "pane{SEP}#{{session_name}}{SEP}#{{window_index}}{SEP}#{{pane_active}}{SEP}#{{pane_current_path}}{SEP}#{{pane_current_command}}"
    );
    let mut out = String::new();
    for line in query_lines(socket, &["list-windows", "-a", "-F", &wfmt]) {
        out.push_str(&line);
        out.push('\n');
    }
    for line in query_lines(socket, &["list-panes", "-a", "-F", &pfmt]) {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// Write `text` as a timestamped snapshot plus the stable `last.txt` restore
/// reads by default. Returns the snapshot path.
fn write_snapshot(text: &str) -> Option<PathBuf> {
    let d = dir()?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let snap = d.join(format!("{stamp}.txt"));
    std::fs::write(&snap, text).ok()?;
    std::fs::write(d.join("last.txt"), text).ok()?;
    Some(snap)
}

fn save(socket: &str) -> i32 {
    let out = capture(socket);
    if out.is_empty() {
        eprintln!("resurrect: nothing to save (no server?)");
        return 1;
    }
    let Some(snap) = write_snapshot(&out) else {
        eprintln!("resurrect: write failed");
        return 1;
    };
    let wins = out.lines().filter(|l| l.starts_with("win")).count();
    let panes = out.lines().filter(|l| l.starts_with("pane")).count();
    println!("saved {wins} windows, {panes} panes -> {}", snap.display());
    0
}

// ---- autosave daemon -----------------------------------------------------

/// Background loop that re-saves every `interval` seconds (continuum-style).
/// Runs top-level (the only context where the nested `list-*` queries work),
/// so it is spawned detached — from `@ztmux-resurrect-auto`'s client-attached
/// hook, or by hand. A per-socket pidfile keeps a single daemon per server.
fn autosave(socket: &str) -> i32 {
    let interval = std::env::args()
        .collect::<Vec<_>>()
        .iter()
        .position(|a| a == "autosave")
        .and_then(|i| std::env::args().nth(i + 1))
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|n| *n >= 5)
        .unwrap_or(900);

    let Some(d) = dir() else {
        return 1;
    };
    let pidfile = d.join(format!("autosave-{}.pid", socket.replace(['/', ':'], "_")));
    if daemon_alive(&pidfile) {
        return 0; // one already running for this socket
    }
    let _ = std::fs::write(&pidfile, std::process::id().to_string());

    // Restore-on-start: the pidfile guard means this daemon is the first for this
    // server, so this runs once — at the first attach after the server started
    // (continuum's "restore on fresh server start"). restore() only fills in
    // sessions that aren't already live, so it never clobbers anything.
    let restore_on_start = query_lines(
        socket,
        &["show-options", "-gqv", "@ztmux-resurrect-restore"],
    )
    .first()
    .is_some_and(|v| matches!(v.trim(), "on" | "1" | "true" | "yes"));
    if restore_on_start {
        restore(socket);
    }

    loop {
        std::thread::sleep(std::time::Duration::from_secs(interval));
        // Stop if a newer daemon took over the pidfile, or the server is gone.
        if !owns_pidfile(&pidfile) {
            return 0;
        }
        let out = capture(socket);
        if out.is_empty() {
            let _ = std::fs::remove_file(&pidfile);
            return 0; // server exited
        }
        let _ = write_snapshot(&out);
    }
}

/// Whether a live autosave daemon (other than us) holds the pidfile.
fn daemon_alive(pidfile: &std::path::Path) -> bool {
    match std::fs::read_to_string(pidfile)
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
    {
        Some(pid) if pid != std::process::id() as i32 => {
            // kill(pid, 0): 0 means the process exists.
            unsafe { libc::kill(pid, 0) == 0 }
        }
        _ => false,
    }
}

/// Whether the pidfile still names us (a newer daemon overwrites it).
fn owns_pidfile(pidfile: &std::path::Path) -> bool {
    std::fs::read_to_string(pidfile)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        == Some(std::process::id())
}

// ---- restore -------------------------------------------------------------

/// Parse the save file into per-window records with their panes attached.
fn parse(text: &str) -> Vec<Win> {
    let mut wins: Vec<Win> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(SEP).collect();
        match f.first().copied() {
            Some("win") if f.len() >= 6 => wins.push(Win {
                session: f[1].to_string(),
                index: f[2].to_string(),
                name: f[3].to_string(),
                layout: f[4].to_string(),
                active: f[5] == "1",
                panes: Vec::new(),
            }),
            Some("pane") if f.len() >= 6 => {
                if let Some(w) = wins
                    .iter_mut()
                    .find(|w| w.session == f[1] && w.index == f[2])
                {
                    w.panes.push(Pane {
                        active: f[3] == "1",
                        cwd: f[4].to_string(),
                        command: f[5].to_string(),
                    });
                }
            }
            _ => {}
        }
    }
    wins
}

fn restore(socket: &str) -> i32 {
    let args: Vec<String> = std::env::args().collect();
    let run_cmds = args.iter().any(|a| a == "--run" || a == "-r");
    // Optional explicit file after `restore` (not a flag); else `last.txt`.
    let named = args
        .iter()
        .position(|a| a == "restore")
        .and_then(|i| args.get(i + 1))
        .filter(|s| !s.starts_with('-'));
    let Some(d) = dir() else {
        eprintln!("resurrect: cannot find $HOME/.ztmux/resurrect");
        return 1;
    };
    let path = match named {
        Some(n) => {
            let p = PathBuf::from(n);
            if p.is_absolute() { p } else { d.join(n) }
        }
        None => d.join("last.txt"),
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("resurrect: cannot read {}", path.display());
        return 1;
    };
    let wins = parse(&text);
    if wins.is_empty() {
        eprintln!("resurrect: nothing to restore in {}", path.display());
        return 1;
    }

    let existing: std::collections::HashSet<String> =
        query_lines(socket, &["list-sessions", "-F", "#{session_name}"])
            .into_iter()
            .collect();

    let mut created_sessions: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut restored = 0usize;
    for w in &wins {
        if existing.contains(&w.session) {
            continue; // never clobber a live session
        }
        let target = format!("{}:{}", w.session, w.index);
        let first_cwd = w.panes.first().map_or("", |p| p.cwd.as_str());
        if created_sessions.insert(w.session.clone()) {
            // First window of a new session creates the session (detached).
            let _ = query_lines(
                socket,
                &[
                    "new-session",
                    "-d",
                    "-s",
                    &w.session,
                    "-n",
                    &w.name,
                    "-c",
                    first_cwd,
                ],
            );
        } else {
            let _ = query_lines(
                socket,
                &[
                    "new-window",
                    "-t",
                    &format!("{}:", w.session),
                    "-n",
                    &w.name,
                    "-c",
                    first_cwd,
                ],
            );
        }
        // Add the remaining panes (the window already has one), each in its cwd.
        for p in w.panes.iter().skip(1) {
            let _ = query_lines(socket, &["split-window", "-t", &target, "-c", &p.cwd]);
        }
        // Restore the exact geometry, then optionally re-run each pane's command.
        if !w.layout.is_empty() {
            let _ = query_lines(socket, &["select-layout", "-t", &target, &w.layout]);
        }
        if run_cmds {
            for (i, p) in w.panes.iter().enumerate() {
                if !p.command.is_empty() && p.command != "zsh" && p.command != "bash" {
                    let pt = format!("{}:{}.{}", w.session, w.index, i);
                    let _ = query_lines(socket, &["send-keys", "-t", &pt, &p.command, "Enter"]);
                }
            }
        }
        // Re-select the active pane of this window.
        if let Some(ai) = w.panes.iter().position(|p| p.active) {
            let _ = query_lines(
                socket,
                &[
                    "select-pane",
                    "-t",
                    &format!("{}:{}.{}", w.session, w.index, ai),
                ],
            );
        }
        restored += 1;
    }

    // Re-select each restored session's active window.
    for w in wins.iter().filter(|w| w.active) {
        if created_sessions.contains(&w.session) {
            let _ = query_lines(
                socket,
                &["select-window", "-t", &format!("{}:{}", w.session, w.index)],
            );
        }
    }
    println!(
        "restored {restored} windows across {} sessions",
        created_sessions.len()
    );
    0
}

// ---- list ----------------------------------------------------------------

fn list() -> i32 {
    let Some(d) = dir() else {
        eprintln!("resurrect: cannot find $HOME/.ztmux/resurrect");
        return 1;
    };
    let Ok(rd) = std::fs::read_dir(&d) else {
        return 0;
    };
    let mut names: Vec<String> = rd
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".txt"))
        .collect();
    names.sort();
    if names.is_empty() {
        println!("no saved snapshots in {}", d.display());
    } else {
        println!("snapshots in {}:", d.display());
        for n in names {
            println!("  {n}");
        }
    }
    0
}
