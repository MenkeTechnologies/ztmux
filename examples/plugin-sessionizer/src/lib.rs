//! `plugin-sessionizer` — one directory, one session, one key.
//!
//! The well-known `tmux-sessionizer` workflow (a project directory becomes a
//! named session you switch to, creating it on first use) written as a native
//! plugin instead of a shell script. It exists to show the other half of the
//! host API: a plugin does not have to *be* the feature, it can **drive the
//! server** by handing it tmux command text through [`Host::run`].
//!
//! ```tmux
//! znative load path:examples/plugin-sessionizer
//! bind C-o command-prompt -p "project:" "sessionizer %%"
//! ```
//!
//! ```text
//! sessionizer ~/src/ztmux        # switch to session "ztmux", creating it there
//! sessionizer -n api ~/src/x     # name it "api" instead
//! sessionizer -d ~/src/x         # create it, but stay where you are
//! ```
//!
//! What it demonstrates:
//!
//! * **flags parsed by tmux itself** — the `dn:` template is handed to the
//!   server at registration, so `args_parse` validates the command line and
//!   [`Ctx::has`] / [`Ctx::arg`] read the result. The plugin never writes an
//!   argument parser.
//! * **`run` is the whole command language** — `new-session`, `switch-client`,
//!   anything. Each call parses and queues, in the order the calls were made.
//! * **the format engine is the read path** — `run` queues commands and never
//!   reports what they found, so to *ask* the server something a plugin
//!   expands a format: `#{S:…}` repeats its body once per session, which is
//!   how [`sessions`] gets the list to test against.
//! * **quoting is the caller's job** — a path with a space in it has to reach
//!   the parser as one token, so [`quote`] wraps it the way tmux's own
//!   `list-keys` output does.

use std::os::raw::c_int;
use std::path::Path;

// The ABI, copied in. A plugin outside this repo copies
// `plugin-abi/ztnative.rs` into its own `src/`; this one points at the
// in-tree original so the examples cannot drift from it.
#[path = "../../../plugin-abi/ztnative.rs"]
mod ztnative;

use crate::ztnative::{Args, Ctx, Host};

/// Wrap `s` as a single-quoted tmux token. tmux's parser takes a single-quoted
/// string literally, so the only character that needs care is the quote
/// itself: close, escape one, reopen.
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// tmux session names cannot contain `.` or `:` (they are target syntax), so a
/// directory called `foo.bar` becomes the session `foo_bar`.
fn session_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().replace(['.', ':'], "_"))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "session".to_string())
}

/// Expand a leading `~/`, so the command is usable from the command prompt
/// where no shell has been near the argument.
fn expand(path: &str) -> String {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) if !home.is_empty() => {
            Path::new(&home).join(rest).to_string_lossy().into_owned()
        }
        _ => path.to_string(),
    }
}

/// Every session the server currently has, read through the format engine.
/// `#{S:…}` expands its body once per session, so this comes back as
/// `work:api:notes:` — and `:` is a safe separator because tmux rewrites it to
/// `_` in any session name (`session_check_name`).
fn sessions(host: &Host, ctx: &Ctx) -> Vec<String> {
    let Some(list) = host.format_expand(ctx, "#{S:#{session_name}:}") else {
        return Vec::new();
    };
    list.split(':')
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

fn sessionizer(host: &Host, ctx: &Ctx, args: &Args) -> c_int {
    let Some(target) = args.rest().first() else {
        host.error(ctx, "usage: sessionizer [-d] [-n name] directory");
        return 1;
    };
    let dir = expand(target);
    let path = Path::new(&dir);
    if !path.is_dir() {
        host.error(ctx, &format!("sessionizer: {dir}: not a directory"));
        return 1;
    }
    let name = ctx.arg("n").unwrap_or_else(|| session_name(path));

    // Ask before creating. `new-session -A` would be the shorter way to say
    // "create or attach", but on an existing session it *attaches the client*
    // (cmd-new-session.c hands off to cmd_attach_session), which would ignore
    // this command's own `-d`. Testing first keeps the two flags honest.
    if !sessions(host, ctx).contains(&name)
        && !host.run(
            ctx,
            &format!("new-session -d -s {} -c {}", quote(&name), quote(&dir)),
        )
    {
        host.error(ctx, "sessionizer: could not queue new-session");
        return 1;
    }
    if !ctx.has('d') && !host.run(ctx, &format!("switch-client -t {}", quote(&name))) {
        host.error(ctx, "sessionizer: could not queue switch-client");
        return 1;
    }
    0
}

declare_plugin! {
    name: "sessionizer",
    version: "0.1.0",
    commands: {
        "sessionizer" => {
            alias: "szr",
            template: "dn:",
            usage: "[-d] [-n name] directory",
            handler: sessionizer,
        },
    },
}
