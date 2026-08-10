//! Original ztmux extensions — features with no upstream tmux counterpart.
//!
//! Code under `src/extensions/` is deliberately NOT a port of tmux and is
//! therefore exempt from the anti-drift gate (see
//! `tests/ported_fn_names_match_c.rs`), which only holds `src/ported`-style
//! ported code to C-name fidelity.

/// The ztmux extension subcommands (`ztmux <name>`), the single source of truth
/// for both the CLI dispatch in `tmux.rs` and the floating command-prompt
/// palette's candidate list in `ratatui_ui::prompt_candidate_list`. (The ported
/// prompt completes only what `prompt.c` completes — commands and
/// `command-alias` entries — so this list is extension chrome, not a port.)
/// Keep sorted for readability.
pub(crate) const EXTENSION_COMMANDS: &[&str] = &[
    "active",
    "age",
    "ahead",
    "alerts",
    "autoname",
    "banner",
    "bcast",
    "borders",
    "buffers",
    "busy",
    "changes",
    "clearall",
    "cmd",
    "commit",
    "conflicts",
    "connected",
    "constrain",
    "control",
    "cwd",
    "dashboard",
    "dead",
    "dedup",
    "density",
    "destroy",
    "detached",
    "disk",
    "doctor",
    "elapsed",
    "env",
    "equalize",
    "events",
    "fanout",
    "finder",
    "focus",
    "git",
    "gone",
    "graph",
    "grep",
    "groups",
    "history",
    "hooks",
    "idle",
    "info",
    "input",
    "keys",
    "keytable",
    "layout",
    "layouts",
    "limit",
    "linked",
    "autolock",
    "marks",
    "mem",
    "modal",
    "mode",
    "monitor",
    "mouse",
    "named",
    "nested",
    "net",
    "open",
    "peek",
    "pick",
    "piped",
    "ports",
    "project",
    "prune",
    "ps",
    "pstree",
    "readonly",
    "recent",
    "repl",
    "remain",
    "remote",
    "resurrect",
    "revive",
    "retitle",
    "shells",
    "size",
    "snapshot",
    "solo",
    "sessions",
    "ssh",
    "stack",
    "stash",
    "startcmd",
    "state",
    "stats",
    "status",
    "submodules",
    "switcher",
    "sync",
    "tabs",
    "tag",
    "term",
    "titlebar",
    "titles",
    "tree",
    "triggers",
    "tty",
    "usage",
    "user",
    "utf8",
    "vcs",
    "verbs",
    "viewers",
    "visual",
    "watch",
    "who",
    "winsize",
    "worktree",
    "writable",
    "zoom",
];

/// Refuse to start a full-screen subcommand when there is no terminal to draw
/// on, returning the exit code the caller should hand back.
///
/// `ratatui::init` panics if it cannot put the terminal into raw mode, which is
/// what happens when the output is a pipe or a file — so `ztmux dashboard |
/// cat`, or any of these run from a script, died with a Rust panic and an exit
/// code of 101 and printed nothing, because the panic hook had already reset
/// the terminal. Both ends are checked: the UI draws to stdout and reads keys
/// from stdin.
pub(crate) fn require_terminal(name: &str) -> Result<(), i32> {
    use std::io::IsTerminal;

    if std::io::stdout().is_terminal() && std::io::stdin().is_terminal() {
        return Ok(());
    }
    eprintln!("ztmux: {name}: not a terminal");
    Err(1)
}

pub(crate) mod active;
pub(crate) mod age;
pub(crate) mod ahead;
pub(crate) mod alerts;
pub(crate) mod autoname;
pub(crate) mod banner;
pub(crate) mod bcast;
pub(crate) mod borders;
pub(crate) mod buffers;
pub(crate) mod busy;
pub(crate) mod changes;
pub(crate) mod clear;
pub(crate) mod cmd;
pub(crate) mod commit;
pub(crate) mod completion_spec;
pub(crate) mod conflicts;
pub(crate) mod connected;
pub(crate) mod constrain;
pub(crate) mod control;
pub(crate) mod cwd;
pub(crate) mod dashboard;
pub(crate) mod dead;
pub(crate) mod dedup;
pub(crate) mod density;
pub(crate) mod destroy;
pub(crate) mod detached;
pub(crate) mod diagnostics;
pub(crate) mod disk;
pub(crate) mod doctor;
pub(crate) mod elapsed;
pub(crate) mod env;
pub(crate) mod equalize;
pub(crate) mod events;
pub(crate) mod fanout;
pub(crate) mod find;
pub(crate) mod float;
pub(crate) mod focus;
pub(crate) mod git;
pub(crate) mod gitcmd;
pub(crate) mod gone;
pub(crate) mod graph;
pub(crate) mod grep;
pub(crate) mod groups;
pub(crate) mod help;
pub(crate) mod history;
pub(crate) mod hooks;
pub(crate) mod idle;
pub(crate) mod info;
pub(crate) mod input;
pub(crate) mod keys;
pub(crate) mod keytable;
pub(crate) mod layout;
pub(crate) mod layouts;
pub(crate) mod limit;
pub(crate) mod linked;
pub(crate) mod lock;
pub(crate) mod marks;
pub(crate) mod mem;
pub(crate) mod modal;
pub(crate) mod mode;
pub(crate) mod monitor;
pub(crate) mod mouse;
pub(crate) mod named;
pub(crate) mod nested;
pub(crate) mod net;
pub(crate) mod open;
pub(crate) mod peek;
pub(crate) mod pick;
pub(crate) mod piped;
pub(crate) mod ports;
pub(crate) mod procstat;
pub(crate) mod proctree;
pub(crate) mod project;
pub(crate) mod prompt_complete;
pub(crate) mod prune;
pub(crate) mod ps;
pub(crate) mod pstree;
pub(crate) mod ratatui_ui;
pub(crate) mod readonly;
pub(crate) mod recent;
pub(crate) mod remain;
pub(crate) mod remote;
pub(crate) mod repl;
pub(crate) mod respawn;
pub(crate) mod resurrect;
pub(crate) mod retitle;
pub(crate) mod sessions;
pub(crate) mod shell;
pub(crate) mod shells;
pub(crate) mod size;
pub(crate) mod snapshot;
pub(crate) mod socket;
pub(crate) mod solo;
pub(crate) mod ssh;
pub(crate) mod stack;
pub(crate) mod start;
pub(crate) mod stash;
pub(crate) mod state;
pub(crate) mod stats;
pub(crate) mod status;
pub(crate) mod structured;
pub(crate) mod submodules;
pub(crate) mod switch;
pub(crate) mod sync;
pub(crate) mod tabs;
pub(crate) mod tag;
pub(crate) mod term;
pub(crate) mod titlebar;
pub(crate) mod titles;
pub(crate) mod tmux_query;
pub(crate) mod tree;
pub(crate) mod triggers;
pub(crate) mod tty;
pub(crate) mod usage;
pub(crate) mod user;
pub(crate) mod utf8;
pub(crate) mod vcs;
pub(crate) mod verbs;
pub(crate) mod viewers;
pub(crate) mod visual;
pub(crate) mod watch;
pub(crate) mod who;
pub(crate) mod winsize;
pub(crate) mod worktree;
pub(crate) mod writable;
pub(crate) mod zoom;
