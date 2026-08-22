//! `ztmux resurrect` — save and restore the whole server across restarts.
//!
//! Zellij persists sessions so they survive a restart/reboot; tmux does not.
//! `resurrect save` walks every session → window → pane and writes its shape —
//! window names and layouts, and each pane's working directory, running command
//! and full command line — to `~/.ztmux/resurrect/`. `resurrect restore` reads
//! it back and recreates the sessions: windows at their saved indexes, the right
//! number of panes (each in its saved cwd), and the exact tiled geometry via the
//! saved layout string.
//!
//! Restore works pane by pane, the way tmux-resurrect does: a pane that is
//! already live is left alone, a missing pane is split into its existing window,
//! a missing window is created in its existing session, and only a missing
//! session is created from scratch. Running it against a live server therefore
//! fills in what is gone instead of doing nothing, and running it twice is a
//! no-op.
//!
//! A shell pane restores perfectly (same cwd). An arbitrary running program
//! cannot be resumed, so panes come back as a shell in the right directory and
//! the saved command line is then re-sent for the programs on the restore list
//! (`@ztmux-resurrect-processes`, defaulting to the editors/pagers upstream
//! uses); `--run` re-sends every saved command line regardless of the list.
//!
//! Subcommands: `save` (default), `restore [file] [--run]`, `list`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::tmux_query::query_lines;

/// Field separator in the save file (unit separator: never appears in names,
/// paths or layout strings).
const SEP: char = '\u{1f}';

/// Programs whose command line is re-sent on restore without `--run`, matching
/// tmux-resurrect's `@resurrect-default-processes`.
const DEFAULT_PROCESSES: &[&str] = &[
    "vi", "vim", "view", "nvim", "emacs", "man", "less", "more", "tail", "top", "htop", "irssi",
    "weechat", "mutt",
];

struct Pane {
    /// Saved `#{pane_index}` — the target suffix, which already accounts for
    /// `pane-base-index`, so it is used verbatim rather than a loop counter.
    index: String,
    active: bool,
    cwd: String,
    /// `#{pane_current_command}` — the process name only (`nvim`, `claude`).
    command: String,
    /// The child process's full argv (`nvim -S Session.vim`), empty when the
    /// pane sits at an idle shell prompt with no child.
    full: String,
    /// `#{pane_floating_flag}` — a floating pane is rebuilt with `new-pane`,
    /// not by splitting the tiled tree.
    floating: bool,
    /// `#{pane_id}` (`%3`), which ties the pane to its cell in the layout's
    /// floating suffix. Meaningless after a restart, used only within one file.
    id: String,
}

/// A floating pane's saved cell: the geometry `new-pane` has to reproduce.
struct Float {
    sx: u32,
    sy: u32,
    xoff: u32,
    yoff: u32,
    /// The `%N` pane this cell belonged to, as written in the layout.
    id: String,
}

struct Win {
    session: String,
    index: String,
    name: String,
    /// `#{window_layout}` with its floating cells taken out and the checksum
    /// recomputed, so `select-layout` accepts it (see `split_layout`).
    layout: String,
    /// The floating cells that were removed, topmost first.
    floats: Vec<Float>,
    active: bool,
    panes: Vec<Pane>,
}

pub(crate) fn run(socket: &str) -> i32 {
    let args = super::verb_args();
    match op_arg(args).as_deref() {
        Some("restore") => restore(socket, args),
        Some("list") => list(),
        Some("autosave") => autosave(socket, args),
        _ => save(socket),
    }
}

/// The subcommand word after the verb.
fn op_arg(args: &[String]) -> Option<String> {
    args.first().filter(|s| !s.starts_with('-')).cloned()
}

/// `~/.ztmux/resurrect`, created if missing.
fn dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let d = PathBuf::from(home).join(".ztmux").join("resurrect");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

// ---- save ----------------------------------------------------------------

/// Shells whose name in `#{pane_current_command}` means the pane is sitting at
/// a prompt with nothing to restore.
const SHELLS: &[&str] = &["sh", "bash", "zsh", "fish", "dash", "ksh", "csh", "tcsh"];

/// Every process's full argv, keyed both by its own pid and by its parent's.
///
/// Port of tmux-resurrect's default `ps` save-command strategy
/// (`save_command_strategies/ps.sh`: `ps -ao ppid,args`, keep the line whose
/// ppid is the pane's pid, drop the ppid column). Upstream runs `ps` once per
/// pane and prefix-matches the pid with `grep ^`; one call and an exact integer
/// match is both faster and correct for pids that share a prefix. The by-pid map
/// is the part upstream lacks: it covers a pane that runs its program directly
/// (`split-window "tail -f log"`), where the pane's own process is the program
/// and there is no child to find.
struct Commands {
    by_parent: HashMap<String, String>,
    by_pid: HashMap<String, String>,
}

fn commands() -> Commands {
    let mut by_parent = HashMap::new();
    let mut by_pid = HashMap::new();
    let Ok(out) = std::process::Command::new("ps")
        .args(["-ao", "pid=,ppid=,args="])
        .output()
    else {
        return Commands { by_parent, by_pid };
    };
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        // `ps` right-aligns the id columns, so the gaps are runs of spaces.
        let Some((pid, rest)) = line.trim_start().split_once(char::is_whitespace) else {
            continue;
        };
        let Some((ppid, args)) = rest.trim_start().split_once(char::is_whitespace) else {
            continue;
        };
        if pid.parse::<u32>().is_err() || ppid.parse::<u32>().is_err() {
            continue;
        }
        let args = args.trim().to_string();
        by_pid.insert(pid.to_string(), args.clone());
        // First child wins, like the first line upstream's grep pipeline keeps.
        by_parent.entry(ppid.trim().to_string()).or_insert(args);
    }
    Commands { by_parent, by_pid }
}

/// Capture the whole server as the save-file text (window + pane lines).
fn capture(socket: &str) -> String {
    let wfmt = format!(
        "win{SEP}#{{session_name}}{SEP}#{{window_index}}{SEP}#{{window_name}}{SEP}#{{window_layout}}{SEP}#{{window_active}}"
    );
    let pfmt = format!(
        "pane{SEP}#{{session_name}}{SEP}#{{window_index}}{SEP}#{{pane_index}}{SEP}#{{pane_active}}{SEP}#{{pane_current_path}}{SEP}#{{pane_current_command}}{SEP}#{{pane_floating_flag}}{SEP}#{{pane_id}}{SEP}#{{pane_pid}}"
    );
    let mut out = String::new();
    for line in query_lines(socket, &["list-windows", "-a", "-F", &wfmt]) {
        out.push_str(&line);
        out.push('\n');
    }
    let cmds = commands();
    for line in query_lines(socket, &["list-panes", "-a", "-F", &pfmt]) {
        // Trade the trailing pane pid for the full command line it resolves to,
        // so the file never carries a pid that is meaningless after a restart.
        let Some((head, pid)) = line.rsplit_once(SEP) else {
            continue;
        };
        // `#{pane_current_command}` sits three fields back (…, command,
        // floating, id) and says whether the pane is just a shell.
        let bare = head
            .rsplit(SEP)
            .nth(2)
            .unwrap_or("")
            .trim_start_matches('-');
        let full = cmds.by_parent.get(pid).map_or_else(
            || {
                if SHELLS.contains(&bare) {
                    ""
                } else {
                    cmds.by_pid.get(pid).map_or("", String::as_str)
                }
            },
            String::as_str,
        );
        out.push_str(head);
        out.push(SEP);
        out.push_str(full);
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
fn autosave(socket: &str, args: &[String]) -> i32 {
    let interval = args
        .iter()
        .position(|a| a == "autosave")
        .and_then(|i| args.get(i + 1))
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
    // (continuum's "restore on fresh server start"). restore() only creates what
    // is missing, so it never clobbers a live pane.
    let restore_on_start = query_lines(
        socket,
        &["show-options", "-gqv", "@ztmux-resurrect-restore"],
    )
    .first()
    .is_some_and(|v| matches!(v.trim(), "on" | "1" | "true" | "yes"));
    if restore_on_start {
        restore(socket, &[]);
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
///
/// Both pane layouts are accepted: the current 8-field line (`pane`, session,
/// window, pane index, active, cwd, command, full command) and the original
/// 6-field line that carried no pane index and no command line — snapshots
/// written before the format grew still restore, with the pane's position
/// standing in for its index.
fn parse(text: &str) -> Vec<Win> {
    let mut wins: Vec<Win> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(SEP).collect();
        match f.first().copied() {
            Some("win") if f.len() >= 6 => {
                let (layout, floats) = split_layout(f[4]);
                wins.push(Win {
                    session: f[1].to_string(),
                    index: f[2].to_string(),
                    name: f[3].to_string(),
                    layout,
                    floats,
                    active: f[5] == "1",
                    panes: Vec::new(),
                });
            }
            Some("pane") if f.len() >= 6 => {
                let Some(w) = wins
                    .iter_mut()
                    .find(|w| w.session == f[1] && w.index == f[2])
                else {
                    continue;
                };
                let pane = if f.len() >= 10 {
                    Pane {
                        index: f[3].to_string(),
                        active: f[4] == "1",
                        cwd: f[5].to_string(),
                        command: f[6].to_string(),
                        floating: f[7] == "1",
                        id: f[8].to_string(),
                        full: f[9].to_string(),
                    }
                } else if f.len() >= 8 {
                    Pane {
                        index: f[3].to_string(),
                        active: f[4] == "1",
                        cwd: f[5].to_string(),
                        command: f[6].to_string(),
                        floating: false,
                        id: String::new(),
                        full: f[7].to_string(),
                    }
                } else {
                    Pane {
                        index: w.panes.len().to_string(),
                        active: f[3] == "1",
                        cwd: f[4].to_string(),
                        command: f[5].to_string(),
                        floating: false,
                        id: String::new(),
                        full: String::new(),
                    }
                };
                w.panes.push(pane);
            }
            _ => {}
        }
    }
    wins
}

/// The rotating checksum a layout string carries in its first four characters.
/// Port of `layout_checksum` (`vendor/tmux/layout-custom.c:47`) — needed because
/// taking the floating cells out of a layout invalidates the saved one.
fn layout_checksum(s: &str) -> u16 {
    let mut csum: u16 = 0;
    for b in s.bytes() {
        csum = (csum >> 1) + ((csum & 1) << 15);
        csum = csum.wrapping_add(u16::from(b));
    }
    csum
}

/// Split a saved `#{window_layout}` into a tiled-only layout string and the
/// floating cells it described.
///
/// A window with a floating pane dumps that pane twice: once inside the tiled
/// tree and again in a trailing `<cell,…>` list. The tiled sizes then no longer
/// add up, so tmux itself rejects the string it just produced —
/// `select-layout "$(display-message -p '#{window_layout}')"` fails with
/// "invalid layout" on tmux 3.7 as well. Restoring the geometry therefore means
/// rebuilding a layout that holds only the tiled panes, and re-creating the
/// floats with `new-pane`, which is what this split feeds.
fn split_layout(layout: &str) -> (String, Vec<Float>) {
    let Some(open) = layout.find('<') else {
        return (layout.to_string(), Vec::new());
    };
    let Some(body) = layout[open + 1..].strip_suffix('>') else {
        return (layout.to_string(), Vec::new());
    };

    // Each floating cell is a leaf: "SXxSY,XOFF,YOFF,PANEID".
    let fields: Vec<&str> = body.split(',').collect();
    let mut floats = Vec::new();
    let mut cells: Vec<String> = Vec::new();
    for c in fields.chunks(4) {
        let [size, xoff, yoff, id] = c else { continue };
        let Some((sx, sy)) = size.split_once('x') else {
            continue;
        };
        let (Ok(sx), Ok(sy), Ok(xoff), Ok(yoff)) = (
            sx.parse::<u32>(),
            sy.parse::<u32>(),
            xoff.parse::<u32>(),
            yoff.parse::<u32>(),
        ) else {
            continue;
        };
        cells.push(c.join(","));
        floats.push(Float {
            sx,
            sy,
            xoff,
            yoff,
            id: (*id).to_string(),
        });
    }
    if floats.is_empty() {
        return (layout.to_string(), Vec::new());
    }

    // Drop the suffix, then the same cells from inside the tiled tree. The pane
    // id makes each cell string unique, so a plain replace is unambiguous.
    let mut tiled = layout[..open].to_string();
    for cell in &cells {
        if let Some(at) = tiled.find(&format!(",{cell}")) {
            tiled.replace_range(at..at + cell.len() + 1, "");
        } else if let Some(at) = tiled.find(&format!("{cell},")) {
            tiled.replace_range(at..at + cell.len() + 1, "");
        }
    }
    // The first four characters are the old checksum of the old body.
    let body = tiled.get(5..).unwrap_or("").to_string();
    (format!("{:04x},{body}", layout_checksum(&body)), floats)
}

/// The live server's sessions, `session:window` windows, and
/// `session:window.pane` panes.
struct Live {
    sessions: HashSet<String>,
    windows: HashSet<String>,
    panes: HashSet<String>,
}

fn live(socket: &str) -> Live {
    let sessions = query_lines(socket, &["list-sessions", "-F", "#{session_name}"])
        .into_iter()
        .collect();
    let windows = query_lines(
        socket,
        &[
            "list-windows",
            "-a",
            "-F",
            "#{session_name}:#{window_index}",
        ],
    )
    .into_iter()
    .collect();
    let panes = query_lines(
        socket,
        &[
            "list-panes",
            "-a",
            "-F",
            "#{session_name}:#{window_index}.#{pane_index}",
        ],
    )
    .into_iter()
    .collect();
    Live {
        sessions,
        windows,
        panes,
    }
}

/// Resolve the file to restore from: an explicit name (absolute, or relative to
/// the snapshot directory, with `.txt` filled in when the bare name misses), or
/// `last.txt`.
fn snapshot_path(d: &std::path::Path, named: Option<&String>) -> PathBuf {
    let Some(n) = named else {
        return d.join("last.txt");
    };
    let p = PathBuf::from(n);
    let p = if p.is_absolute() { p } else { d.join(n) };
    if p.exists() {
        return p;
    }
    // `restore last` should find `last.txt`.
    let with_ext = p.with_file_name(format!(
        "{}.txt",
        p.file_name().unwrap_or_default().to_string_lossy()
    ));
    if with_ext.exists() { with_ext } else { p }
}

/// Push `-c <dir>` only for a directory that was actually captured: an empty
/// `#{pane_current_path}` (a pane whose process died, or one running under
/// `sudo`) would otherwise make the create fail outright.
fn cwd_args<'a>(args: &mut Vec<&'a str>, cwd: &'a str) {
    if !cwd.is_empty() {
        args.push("-c");
        args.push(cwd);
    }
}

/// Create a detached session for its first saved window, then move that window
/// to the saved index if `base-index` put it somewhere else (upstream
/// `new_session`).
fn new_session(socket: &str, w: &Win, cwd: &str) {
    let mut args = vec!["new-session", "-d", "-s", &w.session];
    cwd_args(&mut args, cwd);
    let _ = query_lines(socket, &args);
    let created = query_lines(
        socket,
        &["list-windows", "-t", &w.session, "-F", "#{window_index}"],
    );
    if let Some(first) = created.first()
        && first != &w.index
    {
        let _ = query_lines(
            socket,
            &[
                "move-window",
                "-s",
                &format!("{}:{first}", w.session),
                "-t",
                &format!("{}:{}", w.session, w.index),
            ],
        );
    }
}

/// Create a window at its saved index in an existing session (upstream
/// `new_window`).
fn new_window(socket: &str, w: &Win, cwd: &str) {
    let target = format!("{}:{}", w.session, w.index);
    let mut args = vec!["new-window", "-d", "-t", target.as_str()];
    cwd_args(&mut args, cwd);
    let _ = query_lines(socket, &args);
}

/// Split one more pane into an existing window, then shrink every other pane to
/// give the next split room (upstream `new_pane`'s `resize-pane -U 999`).
///
/// Without the shrink each split halves the active pane, so an 80x24 window runs
/// out of space after four splits — a twelve-pane window came back with four
/// panes and a layout string that no longer applied.
fn new_pane(socket: &str, target: &str, cwd: &str) {
    let mut args = vec!["split-window", "-t", target];
    cwd_args(&mut args, cwd);
    let _ = query_lines(socket, &args);
    let _ = query_lines(socket, &["resize-pane", "-t", target, "-U", "999"]);
}

/// Whether `new-pane` trims a border off the size it is given: it takes an
/// outer size, so it subtracts two rows/columns and shifts the offset by one
/// unless `pane-border-lines` is `none` (`layout_floating_args_parse`,
/// `src/ported/layout.rs`). Restoring an exact cell means adding that back.
fn border_offset(socket: &str) -> u32 {
    let lines = query_lines(socket, &["show-options", "-gv", "pane-border-lines"]);
    u32::from(lines.first().map(|s| s.trim()) != Some("none"))
}

/// Re-create a floating pane at the exact cell the layout recorded.
fn new_float(socket: &str, target: &str, f: &Float, cwd: &str, border: u32) {
    let (sx, sy) = (f.sx + 2 * border, f.sy + 2 * border);
    let (ox, oy) = (f.xoff.saturating_sub(border), f.yoff.saturating_sub(border));
    let (sx, sy, ox, oy) = (
        sx.to_string(),
        sy.to_string(),
        ox.to_string(),
        oy.to_string(),
    );
    let mut args = vec![
        "new-pane", "-d", "-t", target, "-x", &sx, "-y", &sy, "-X", &ox, "-Y", &oy,
    ];
    cwd_args(&mut args, cwd);
    let _ = query_lines(socket, &args);
}

/// Whether a pane's saved command line should be re-sent: everything with
/// `--run`, otherwise only the programs on `@ztmux-resurrect-processes` (or the
/// upstream default list when that option is unset). `:all:` means everything,
/// `false` means nothing — both as in tmux-resurrect.
fn process_filter(socket: &str, run_all: bool) -> Box<dyn Fn(&str) -> bool> {
    if run_all {
        return Box::new(|full: &str| !full.is_empty());
    }
    let opt = query_lines(
        socket,
        &["show-options", "-gqv", "@ztmux-resurrect-processes"],
    )
    .first()
    .map_or(String::new(), |s| s.trim().to_string());
    if opt == "false" {
        return Box::new(|_| false);
    }
    if opt == ":all:" {
        return Box::new(|full: &str| !full.is_empty());
    }
    let mut list: Vec<String> = DEFAULT_PROCESSES.iter().map(|s| (*s).to_string()).collect();
    list.extend(opt.split_whitespace().map(str::to_string));
    Box::new(move |full: &str| {
        let Some(word) = full.split_whitespace().next() else {
            return false;
        };
        // Match the program, not its path, so `/usr/bin/vim file` counts.
        let base = word.rsplit('/').next().unwrap_or(word);
        list.iter().any(|p| {
            // A `~` prefix means "appears anywhere in the command line".
            p.strip_prefix('~')
                .map_or(base == p, |needle| full.contains(needle))
        })
    })
}

fn restore(socket: &str, args: &[String]) -> i32 {
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
    let path = snapshot_path(&d, named);
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("resurrect: cannot read {}", path.display());
        return 1;
    };
    let wins = parse(&text);
    if wins.is_empty() {
        eprintln!("resurrect: nothing to restore in {}", path.display());
        return 1;
    }

    let mut have = live(socket);
    let restorable = process_filter(socket, run_cmds);
    let (mut new_sessions, mut new_windows, mut new_panes, mut kept) = (0usize, 0usize, 0usize, 0);
    // Windows this run actually built, and so may lay out and rename.
    let mut touched: Vec<&Win> = Vec::new();

    for w in &wins {
        let wtarget = format!("{}:{}", w.session, w.index);
        let mut built = false;
        for p in &w.panes {
            let ptarget = format!("{wtarget}.{}", p.index);
            if have.panes.contains(&ptarget) {
                kept += 1; // live pane: never touched, and its process is left alone
                continue;
            }
            if p.floating {
                continue; // rebuilt below, once the tiled panes are laid out
            }
            if have.windows.contains(&wtarget) {
                new_pane(socket, &wtarget, &p.cwd);
                new_panes += 1;
            } else if have.sessions.contains(&w.session) {
                new_window(socket, w, &p.cwd);
                have.windows.insert(wtarget.clone());
                new_windows += 1;
                new_panes += 1;
            } else {
                new_session(socket, w, &p.cwd);
                have.sessions.insert(w.session.clone());
                have.windows.insert(wtarget.clone());
                new_sessions += 1;
                new_windows += 1;
                new_panes += 1;
            }
            have.panes.insert(ptarget.clone());
            built = true;
            // Re-send the saved command line into the pane we just made.
            let full = if p.full.is_empty() {
                p.command.as_str()
            } else {
                p.full.as_str()
            };
            if restorable(full) {
                let _ = query_lines(socket, &["send-keys", "-t", &ptarget, full, "Enter"]);
            }
        }
        if built {
            touched.push(w);
        }
    }

    // Geometry and names, once every tiled pane of the window exists.
    let border = border_offset(socket);
    for w in &touched {
        let target = format!("{}:{}", w.session, w.index);
        if !w.layout.is_empty() {
            let _ = query_lines(socket, &["select-layout", "-t", &target, &w.layout]);
        }
        if !w.name.is_empty() {
            let _ = query_lines(socket, &["rename-window", "-t", &target, &w.name]);
        }
        // Floating panes last, bottom of the stack first so the z-order comes
        // back the way it was saved.
        for f in w.floats.iter().rev() {
            // `#{pane_id}` is `%3`; the layout writes the bare number.
            let Some(p) = w
                .panes
                .iter()
                .find(|p| p.floating && p.id.trim_start_matches('%') == f.id)
            else {
                continue;
            };
            new_float(socket, &target, f, &p.cwd, border);
            new_panes += 1;
            let full = if p.full.is_empty() {
                p.command.as_str()
            } else {
                p.full.as_str()
            };
            if restorable(full) {
                let _ = query_lines(
                    socket,
                    &[
                        "send-keys",
                        "-t",
                        &format!("{target}.{}", p.index),
                        full,
                        "Enter",
                    ],
                );
            }
        }
        if let Some(active) = w.panes.iter().find(|p| p.active) {
            let _ = query_lines(
                socket,
                &["select-pane", "-t", &format!("{target}.{}", active.index)],
            );
        }
    }
    // Each rebuilt session's active window.
    for w in touched.iter().filter(|w| w.active) {
        let _ = query_lines(
            socket,
            &["select-window", "-t", &format!("{}:{}", w.session, w.index)],
        );
    }

    if new_panes == 0 {
        println!("nothing to restore: all {kept} saved panes are already live");
    } else {
        println!(
            "restored {new_panes} panes in {new_windows} windows, {new_sessions} sessions ({kept} already live)"
        );
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A window holding two tiled panes and one floating pane, as
    /// `#{window_layout}` dumps it: the float appears both inside the tiled
    /// tree and in the trailing `<…>` list.
    const WITH_FLOAT: &str = "c346,80x24,0,0[80x12,0,0,0,80x11,0,13,1,40x6,4,2,2]<40x6,4,2,2>";

    #[test]
    fn split_layout_strips_the_float_and_restamps_the_checksum() {
        let (tiled, floats) = split_layout(WITH_FLOAT);
        // Byte-identical to what the server reports for the same two tiled
        // panes once the float is gone — checksum included, which is the part
        // `select-layout` rejects if it is recomputed wrong.
        assert_eq!(tiled, "c195,80x24,0,0[80x12,0,0,0,80x11,0,13,1]");
        assert_eq!(floats.len(), 1);
        let f = &floats[0];
        assert_eq!((f.sx, f.sy, f.xoff, f.yoff), (40, 6, 4, 2));
        assert_eq!(f.id, "2");
    }

    #[test]
    fn split_layout_leaves_a_float_free_layout_alone() {
        let plain = "c195,80x24,0,0[80x12,0,0,0,80x11,0,13,1]";
        let (tiled, floats) = split_layout(plain);
        assert_eq!(tiled, plain);
        assert!(floats.is_empty());
    }

    #[test]
    fn split_layout_keeps_stacking_order_for_several_floats() {
        let (tiled, floats) = split_layout(
            "b306,80x24,0,0[80x12,0,0,0,40x6,4,2,2,80x11,0,13,1,40x6,8,4,3]<40x6,8,4,3,40x6,4,2,2>",
        );
        // Both cells leave the tiled tree, wherever they sat in the child list.
        assert!(!tiled.contains("40x6"), "float still inline: {tiled}");
        assert_eq!(
            floats.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(),
            ["3", "2"],
            "topmost float must stay first so the z-order survives"
        );
    }

    #[test]
    fn parse_reads_both_pane_line_formats() {
        let old = format!(
            "win{SEP}s{SEP}0{SEP}w{SEP}c195,80x24,0,0{SEP}1\n\
             pane{SEP}s{SEP}0{SEP}1{SEP}/tmp{SEP}zsh\n"
        );
        let wins = parse(&old);
        assert_eq!(wins[0].panes.len(), 1);
        // No pane index in the old format: the position stands in for it.
        assert_eq!(wins[0].panes[0].index, "0");
        assert!(wins[0].panes[0].active);
        assert_eq!(wins[0].panes[0].full, "");

        let new = format!(
            "win{SEP}s{SEP}0{SEP}w{SEP}c195,80x24,0,0{SEP}1\n\
             pane{SEP}s{SEP}0{SEP}3{SEP}1{SEP}/tmp{SEP}less{SEP}0{SEP}%7{SEP}less /etc/hosts\n"
        );
        let wins = parse(&new);
        let p = &wins[0].panes[0];
        assert_eq!(p.index, "3");
        assert_eq!(p.id, "%7");
        assert_eq!(p.full, "less /etc/hosts");
        assert!(!p.floating);
    }
}
