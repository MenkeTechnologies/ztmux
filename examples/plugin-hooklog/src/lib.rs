//! `plugin-hooklog` — the server's recent history, kept by hooks and readable
//! from the status line.
//!
//! Hooks, state, and a format variable are one loop: a hook fires in the
//! server and appends to a ring buffer, and `#{plugin_hooklog_last}` reads the
//! newest entry back out on the next redraw. Nothing is written to disk and
//! nothing is forked; the whole thing lives in the plugin's own memory for the
//! lifetime of the server.
//!
//! ```tmux
//! znative load path:examples/plugin-hooklog
//! set -g status-right '#{plugin_hooklog_last}'
//! ```
//!
//! ```text
//! hooklog            # print the log, newest last
//! hooklog -n 5       # only the last five
//! hooklog -c         # clear it
//! ```
//!
//! What it demonstrates:
//!
//! * **hook subscriptions** — nine of them, each an ordinary tmux hook name.
//!   A handler is given the [`Hook`] the notification carried (client, session,
//!   window, window id, pane id) and the empty [`Ctx`], which can still `run`
//!   tmux commands: they queue globally, since a hook has no client of its own.
//! * **hooks feeding a format** — the status line reads plugin state that was
//!   written by a server event, with no polling in between.
//! * **the notification is push, not pull** — a hook is *told* what happened,
//!   and everything recorded below is what the notification itself carried.
//!   Reading server state is a separate motion, through the format engine
//!   (`format_expand`); see `plugin-sessionizer`.

use std::os::raw::c_int;
use std::sync::Mutex;

// The ABI, copied in. A plugin outside this repo copies
// `plugin-abi/ztnative.rs` into its own `src/`; this one points at the
// in-tree original so the examples cannot drift from it.
#[path = "../../../plugin-abi/ztnative.rs"]
mod ztnative;

use crate::ztnative::{Args, Ctx, Hook, Host};

/// How many entries are kept. A status line shows one; the command shows the
/// tail. Bounded so a long-lived server cannot grow the plugin without limit.
const CAPACITY: usize = 64;

/// The log, newest last.
static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Render one notification as a line: the hook name, then whichever of the
/// target fields it actually carried.
fn describe(hook: &Hook) -> String {
    let mut line = hook.name.clone();
    if let Some(session) = &hook.session {
        line.push_str(&format!(" session={session}"));
    }
    if let Some(window) = &hook.window {
        line.push_str(&format!(" window={window}"));
    }
    if let Some(id) = hook.window_id {
        line.push_str(&format!(" @{id}"));
    }
    if let Some(id) = hook.pane_id {
        line.push_str(&format!(" %{id}"));
    }
    if let Some(client) = &hook.client {
        line.push_str(&format!(" client={client}"));
    }
    line
}

/// Every subscription lands here. One handler for nine hooks is the ordinary
/// case: the hook's own name is in the event, so the plugin does not need a
/// function per subscription.
fn record(_host: &Host, _ctx: &Ctx, hook: &Hook) {
    let Ok(mut log) = LOG.lock() else { return };
    if log.len() == CAPACITY {
        log.remove(0);
    }
    log.push(describe(hook));
}

/// `#{plugin_hooklog_last}` — the newest entry, or nothing yet.
fn last(_host: &Host, _key: &str) -> Option<String> {
    LOG.lock().ok()?.last().cloned()
}

/// `#{plugin_hooklog_count}` — how many notifications this server has seen
/// since the plugin loaded (capped at [`CAPACITY`] entries kept, so this is
/// the size of the log, not a lifetime total).
fn count(_host: &Host, _key: &str) -> Option<String> {
    Some(LOG.lock().ok()?.len().to_string())
}

/// `hooklog [-c] [-n count]` — print the log, or clear it.
fn hooklog(host: &Host, ctx: &Ctx, _args: &Args) -> c_int {
    if ctx.has('c') {
        if let Ok(mut log) = LOG.lock() {
            log.clear();
        }
        host.print(ctx, "hooklog: cleared");
        return 0;
    }
    let Ok(log) = LOG.lock() else {
        host.error(ctx, "hooklog: log is poisoned");
        return 1;
    };
    if log.is_empty() {
        host.print(ctx, "hooklog: nothing recorded yet");
        return 0;
    }
    let wanted = ctx
        .arg("n")
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(log.len())
        .min(log.len());
    for line in &log[log.len() - wanted..] {
        host.print(ctx, line);
    }
    0
}

declare_plugin! {
    name: "hooklog",
    version: "0.1.0",
    commands: {
        "hooklog" => { template: "cn:", usage: "[-c] [-n count]", handler: hooklog },
    },
    formats: {
        "plugin_hooklog_last" => last,
        "plugin_hooklog_count" => count,
    },
    hooks: {
        "session-created" => record,
        "session-closed" => record,
        "session-renamed" => record,
        "window-linked" => record,
        "window-unlinked" => record,
        "window-renamed" => record,
        "pane-exited" => record,
        "client-attached" => record,
        "client-detached" => record,
    },
}
