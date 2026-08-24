//! `plugin-continuum` — tmux-continuum's automatic saving, without the daemon.
//!
//! The original keeps a background shell alive for the life of the server: its
//! `*.tmux` file appends a `#(…continuum_status.sh)` to `status-right`, so the
//! status line forks a script on every redraw, and that script decides whether
//! fifteen minutes have passed and shells out to tmux-resurrect if so. The
//! saving works; the mechanism is a polling loop wearing a status-line
//! variable, and it costs a process per redraw whether or not anything changed.
//!
//! Here the trigger is the server telling the plugin what happened. Sessions,
//! windows and panes appearing or disappearing are exactly the events that make
//! a snapshot stale, so those hooks are the save signal, with a minimum
//! interval so a burst of ten `new-window`s is one save and not ten.
//!
//! ```tmux
//! znative load path:examples/plugin-continuum
//! set -g @continuum-save-interval 15   # minutes; 0 disables saving
//! ```
//!
//! | Option | Default | Meaning |
//! | --- | --- | --- |
//! | `@continuum-save-interval` | `15` | minutes between saves; `0` turns it off |
//! | `@continuum-restore` | `off` | restore the last snapshot when the first session appears |
//!
//! What it demonstrates: **hooks as the scheduler**. A plugin cannot ask to be
//! woken on a timer — the ABI has no tick — but it does not need one here,
//! because the state worth saving only changes when a hook fires. It also shows
//! `run` from a hook handler, where there is no client and the queued command
//! goes to the global queue.
//!
//! It drives ztmux's own `resurrect` extension rather than reimplementing
//! save/restore, which is the honest division: the snapshot format belongs to
//! the host, the policy belongs here.

use std::os::raw::c_int;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// The ABI, copied in. A plugin outside this repo copies
// `plugin-abi/ztnative.rs` into its own `src/`; this one points at the
// in-tree original so the examples cannot drift from it.
#[path = "../../../plugin-abi/ztnative.rs"]
mod ztnative;

use crate::ztnative::{Args, Ctx, Hook, Host};

/// When the last save was queued, and how many have been queued in all.
static STATE: Mutex<Option<(Instant, u64)>> = Mutex::new(None);

/// Minutes between saves, from the option. `0` disables.
fn interval(host: &Host) -> Option<Duration> {
    let minutes: u64 = host
        .get_option("@continuum-save-interval")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(15);
    (minutes > 0).then(|| Duration::from_secs(minutes * 60))
}

/// Queue a save, unless one went recently. Returns whether it queued.
fn maybe_save(host: &Host, ctx: &Ctx, force: bool) -> bool {
    let Some(gap) = interval(host) else {
        return false;
    };
    let mut state = match STATE.lock() {
        Ok(s) => s,
        Err(_) => return false,
    };
    if let Some((last, _)) = *state {
        if !force && last.elapsed() < gap {
            return false;
        }
    }
    let count = state.map_or(0, |(_, n)| n) + 1;

    // `run-shell` format-expands its command, so `#{socket_path}` points the
    // client at the server it is being run BY -- which is what makes this work
    // on a `-S`/`-L` socket as well as the default one.
    let queued = host.run(
        ctx,
        "run-shell -b \"ztmux -S '#{socket_path}' resurrect save\"",
    );
    if queued {
        *state = Some((Instant::now(), count));
    }
    queued
}

/// Every subscribed hook lands here: the shape of the server changed, so the
/// snapshot on disk is now behind.
fn changed(host: &Host, ctx: &Ctx, _hook: &Hook) {
    maybe_save(host, ctx, false);
}

/// `continuum [-f]` — report, or force a save now with `-f`.
fn continuum(host: &Host, ctx: &Ctx, _args: &Args) -> c_int {
    if ctx.has('f') {
        return if maybe_save(host, ctx, true) {
            host.print(ctx, "continuum: save queued");
            0
        } else {
            host.error(ctx, "continuum: saving is disabled (@continuum-save-interval 0)");
            1
        };
    }
    match interval(host) {
        None => host.print(ctx, "continuum: disabled (@continuum-save-interval 0)"),
        Some(gap) => {
            let mins = gap.as_secs() / 60;
            let line = match *STATE.lock().unwrap_or_else(|e| e.into_inner()) {
                Some((last, n)) => format!(
                    "continuum: every {mins}m · {n} save(s) queued · last {}s ago",
                    last.elapsed().as_secs()
                ),
                None => format!("continuum: every {mins}m · nothing saved yet"),
            };
            host.print(ctx, &line);
        }
    }
    0
}

/// At load: optionally restore the last snapshot, the way continuum's
/// `@continuum-restore on` does.
fn setup(host: &Host, ctx: &Ctx) {
    if matches!(
        host.get_option("@continuum-restore").as_deref(),
        Some("on" | "yes" | "1" | "true")
    ) {
        host.run(
            ctx,
            "run-shell -b \"ztmux -S '#{socket_path}' resurrect restore\"",
        );
    }
}

declare_plugin! {
    name: "continuum",
    version: "0.1.0",
    commands: {
        "continuum" => { template: "f", usage: "[-f]", handler: continuum },
    },
    hooks: {
        "session-created" => changed,
        "session-closed" => changed,
        "session-renamed" => changed,
        "window-linked" => changed,
        "window-unlinked" => changed,
        "window-renamed" => changed,
        "pane-exited" => changed,
    },
    on_load: setup,
}
