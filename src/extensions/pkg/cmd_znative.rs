//! The `znative` tmux command — the plugin manager's front end.
//!
//! ztmux extension; no tmux C counterpart (tmux has no plugin manager, and
//! TPM lives entirely outside the server as shell script). Registering it in
//! [`crate::cmd_::CMD_TABLE`] rather than bolting it onto the CLI is what
//! makes `znative load owner/repo` work from `.tmux.conf`, the command
//! prompt, a key binding, and `ztmux znative …` alike — one implementation,
//! all four entry points, because they are all the same command queue.
//!
//! This module is the only part of the package manager that touches the
//! server: [`super::commands`] returns an [`Outcome`] of lines to print and
//! shell commands to queue, and everything below turns that into
//! `cmdq_print` output and `run-shell -b` items.

use super::{Outcome, commands};
use crate::*;

/// Usage text, also what `znative help` prints.
const USAGE: &[&str] = &[
    "usage: znative [-n] <command> [args]",
    "",
    "  load [SOURCE...]   load plugin(s); a source not yet in the store is",
    "                     installed first, then loaded (for .tmux.conf).",
    "                     No args loads everything installed. Zero-network",
    "                     once stored. SOURCE: owner/repo, github:o/r,",
    "                     git+URL, path:DIR, any of them with @REF",
    "  add <SOURCE>       install + load a plugin (load self-installs, so add",
    "                     is mainly for installing without a .tmux.conf line)",
    "  remove <NAME>      unload + delete an installed plugin",
    "  list               list installed plugins",
    "  loaded             list the native plugins live in this server",
    "  info <NAME>        show details for one plugin",
    "  update [NAME]      re-resolve + reinstall from the recorded source",
    "  gc [-n]            remove orphan store entries + the git clone cache",
    "  clean              clear scratch caches (git/, cache/, bin/)",
    "  help               this message",
    "",
    "aliases: add=install=i  remove=rm=uninstall  list=ls  info=show",
    "         load=source  update=up=upgrade",
];

pub static CMD_ZNATIVE_ENTRY: cmd_entry = cmd_entry {
    name: "znative",
    alias: None,

    // `-n` is gc's dry run. Subcommands and their arguments are positional,
    // so the command parses its own verbs like every other package manager.
    args: args_parse::new("n", 0, -1, None),
    usage: "[-n] command [arguments]",

    source: cmd_entry_flag::zeroed(),
    target: cmd_entry_flag::zeroed(),

    // Installing and loading plugins is the point of a plugin manager, so
    // `znative load …` from a config file or the CLI has to bring the server
    // up the way `new-session` does rather than failing with "no server".
    flags: cmd_flag::CMD_STARTSERVER,
    exec: cmd_znative_exec,
};

/// Run one `znative` invocation: dispatch the subcommand, print what it
/// produced, and queue the `run-shell -b` items a script plugin loads through.
unsafe fn cmd_znative_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let args = cmd_get_args(self_);
        let argv: Vec<String> = (0..args_count(args))
            .filter_map(|i| {
                let p = args_string(args, i);
                (!p.is_null()).then(|| cstr_to_str(p).to_string())
            })
            .collect();
        let dry_run = args_has(args, 'n');

        let sub = argv.first().map_or("", String::as_str);
        let rest = argv.get(1..).unwrap_or(&[]);

        let result = match sub {
            "add" | "install" | "i" => {
                if rest.is_empty() {
                    return usage_err(item, "add requires a SOURCE");
                }
                each(rest, commands::add)
            }
            "remove" | "rm" | "uninstall" => {
                if rest.is_empty() {
                    return usage_err(item, "remove requires a NAME");
                }
                each(rest, commands::remove)
            }
            "list" | "ls" => commands::list(),
            "loaded" => commands::loaded(),
            "info" | "show" => match rest.first() {
                Some(name) => commands::info(name),
                None => return usage_err(item, "info requires a NAME"),
            },
            // `znative load` with no argument loads everything installed;
            // with arguments it loads each, installing a not-yet-stored
            // source on first use — the `.tmux.conf` line that self-installs.
            "load" | "source" => {
                if rest.is_empty() {
                    commands::load(None)
                } else {
                    each(rest, |s| commands::load(Some(s)))
                }
            }
            "update" | "upgrade" | "up" => commands::update(rest.first().map(String::as_str)),
            "gc" => commands::gc(dry_run || rest.iter().any(|a| a == "--dry-run")),
            "clean" => commands::clean(),
            "help" | "" => {
                for line in USAGE {
                    cmdq_print!(item, "{}", line);
                }
                return cmd_retval::CMD_RETURN_NORMAL;
            }
            other => return usage_err(item, &format!("unknown command '{other}'")),
        };

        let outcome = match result {
            Ok(outcome) => outcome,
            Err(e) => {
                cmdq_error!(item, "znative: {}", e);
                return cmd_retval::CMD_RETURN_ERROR;
            }
        };
        emit(item, outcome)
    }
}

/// Run `f` over every argument, collecting all output and stopping at the
/// first failure — `znative add a b c` should not silently skip `c` because
/// `a` printed something, but it must not pretend `b` succeeded either.
fn each<F>(specs: &[String], f: F) -> super::PkgResult<Outcome>
where
    F: Fn(&str) -> super::PkgResult<Outcome>,
{
    let mut all = Outcome::default();
    for spec in specs {
        all.absorb(f(spec)?);
    }
    Ok(all)
}

/// Print an outcome's lines and queue its script loads, in order.
unsafe fn emit(item: *mut cmdq_item, outcome: Outcome) -> cmd_retval {
    unsafe {
        for line in &outcome.lines {
            cmdq_print!(item, "{}", line);
        }
        // Each queued shell command becomes a `run-shell -b`, chained after
        // the previous one so several plugins load in the order listed. `-b`
        // is TPM's own model: the script drives the server through its own
        // client connection, so it must not block the queue that is running
        // this command.
        let mut after = item;
        for shell in &outcome.queue {
            let command = format!("run-shell -b {}", tmux_quote(shell));
            match cmd_parse_from_string(&command, None) {
                Ok(cmdlist) => {
                    let new_item = cmdq_get_command(cmdlist, cmdq_get_state(item));
                    after = cmdq_insert_after(after, new_item);
                    cmd_list_free(cmdlist);
                }
                Err(error) => {
                    cmdq_error!(item, "znative: {}", _s(error));
                    free_(error);
                    return cmd_retval::CMD_RETURN_ERROR;
                }
            }
        }
        cmd_retval::CMD_RETURN_NORMAL
    }
}

/// Quote a string as one token for tmux's own parser, using tmux's escaper —
/// the same function `list-keys` and `show-options` render with, so whatever
/// it produces re-parses to exactly this string.
fn tmux_quote(s: &str) -> String {
    let owned = cstring_truncating(s.to_string());
    // Safe: `owned` is NUL-terminated and outlives the call; args_escape
    // returns an xmalloc'd string this copies and frees.
    unsafe {
        let escaped = args_escape(owned.as_ptr().cast());
        if escaped.is_null() {
            return s.to_string();
        }
        let out = cstr_to_str(escaped).to_string();
        free_(escaped);
        out
    }
}

/// Report a usage error to the client and fail the command.
unsafe fn usage_err(item: *mut cmdq_item, msg: &str) -> cmd_retval {
    unsafe {
        cmdq_error!(item, "znative: {}", msg);
        for line in USAGE {
            cmdq_print!(item, "{}", line);
        }
        cmd_retval::CMD_RETURN_ERROR
    }
}
