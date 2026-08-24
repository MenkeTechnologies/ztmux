//! `plugin-sensible` — tmux-sensible's settings, applied in-process.
//!
//! The original is a shell script that forks `tmux show-option` once per
//! setting to decide whether the user has already chosen a value, then forks
//! `tmux set-option` for each one it wants to change — about thirty processes
//! at every server start. This does the same decisions with two in-process
//! calls each, and prints what it changed.
//!
//! ```tmux
//! znative load path:examples/plugin-sensible
//! ```
//!
//! The rule that makes tmux-sensible worth copying is **never clobber a
//! choice**: a setting is only applied when the option still holds tmux's own
//! default, so a `.tmux.conf` that sets `history-limit 100000` keeps it. That
//! is the entire logic below, and it is why each entry carries the stock value
//! as well as the wanted one.
//!
//! What it demonstrates: `get_option` / `set_option` as a read-modify-write
//! against live server state, and a plugin whose whole job runs at load —
//! registering one command to report and re-apply, and nothing else.

use std::os::raw::c_int;

// The ABI, copied in. A plugin outside this repo copies
// `plugin-abi/ztnative.rs` into its own `src/`; this one points at the
// in-tree original so the examples cannot drift from it.
#[path = "../../../plugin-abi/ztnative.rs"]
mod ztnative;

use crate::ztnative::{Args, Ctx, Host};

/// `(option, tmux's stock value, what sensible wants)`.
///
/// Taken from tmux-sensible's own `main()`, minus the two entries that cannot
/// be decided from inside the server: `default-command` (needs to know whether
/// `reattach-to-user-namespace` is on PATH) and the key bindings (which need
/// `list-keys` parsing). Those stay the shell plugin's business.
const SETTINGS: &[(&str, &str, &str)] = &[
    ("escape-time", "500", "0"),
    ("history-limit", "2000", "50000"),
    ("display-time", "750", "4000"),
    ("status-interval", "15", "5"),
    ("default-terminal", "screen", "screen-256color"),
];

/// Settings the original applies unconditionally, because there is no
/// meaningful "user already chose this" for them.
const ALWAYS: &[(&str, &str)] = &[("status-keys", "emacs"), ("focus-events", "on")];

/// Apply the set, returning one report line per option.
fn apply(host: &Host) -> Vec<String> {
    let mut out = Vec::new();
    for (name, stock, wanted) in SETTINGS {
        match host.get_option(name) {
            Some(current) if current != *stock => {
                // Either the user chose this or the host's own default differs
                // from the tmux default the rule was written against (ztmux
                // ships escape-time 10 where tmux ships 500). Both mean the
                // same thing here -- leave it alone -- so the line reports the
                // value it is comparing against rather than guessing which.
                out.push(format!("{name}: kept {current} (rule targets {stock})"));
            }
            _ => {
                if host.set_option(name, wanted) {
                    out.push(format!("{name}: {stock} -> {wanted}"));
                } else {
                    out.push(format!("{name}: could not set"));
                }
            }
        }
    }
    for (name, wanted) in ALWAYS {
        if host.set_option(name, wanted) {
            out.push(format!("{name}: {wanted}"));
        }
    }
    out
}

/// `sensible [-v]` — re-apply, and with `-v` say what happened to each option.
/// Re-applying is safe: an option the user has since changed is left alone.
fn sensible(host: &Host, ctx: &Ctx, _args: &Args) -> c_int {
    let report = apply(host);
    if ctx.has('v') {
        for line in &report {
            host.print(ctx, line);
        }
    } else {
        host.print(ctx, &format!("sensible: {} settings applied", report.len()));
    }
    0
}

/// Run once when the plugin loads -- the moment the shell plugin's `*.tmux`
/// file would have been executed.
fn setup(host: &Host, _ctx: &Ctx) {
    apply(host);
}

declare_plugin! {
    name: "sensible",
    version: "0.1.0",
    commands: {
        "sensible" => { template: "v", usage: "[-v]", handler: sensible },
    },
    on_load: setup,
}
