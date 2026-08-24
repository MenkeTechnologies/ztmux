//! `plugin-hello` — the smallest complete native ztmux plugin.
//!
//! It registers one of each thing the [`ztnative`] ABI offers, so it doubles
//! as a check that the whole boundary works:
//!
//! * a tmux **command**, `hello-world`, with a real flag,
//! * a `#{…}` **format**, `plugin_hello_count`,
//! * a **hook** subscription, `session-created`.
//!
//! Build and install it into a running ztmux server:
//!
//! ```text
//! ztmux znative add path:examples/plugin-hello
//! ztmux hello-world -n ztmux
//! ztmux display-message -p '#{plugin_hello_count}'
//! ```

use std::os::raw::c_int;
use std::sync::atomic::{AtomicUsize, Ordering};

// The ABI, copied in. A plugin outside this repo copies
// `plugin-abi/ztnative.rs` into its own `src/`; this one points at the
// in-tree original so the examples cannot drift from it.
#[path = "../../../plugin-abi/ztnative.rs"]
mod ztnative;

use crate::ztnative::{Args, Ctx, Hook, Host};

/// How many times `hello-world` has run in this server — the state behind
/// the `#{plugin_hello_count}` format.
static GREETED: AtomicUsize = AtomicUsize::new(0);

/// `hello-world [-n NAME] [extra...]` — greet, and show what the plugin can
/// see of the server it is running inside.
fn hello(host: &Host, ctx: &Ctx, args: &Args) -> c_int {
    let who = ctx.arg("n").unwrap_or_else(|| "world".to_string());
    GREETED.fetch_add(1, Ordering::Relaxed);

    host.print(ctx, &format!("hello {who}"));
    // Formats expand against the target of the command that is running, so
    // this is the pane the user typed in.
    if let Some(where_) = host.format_expand(ctx, "#{session_name}:#{window_index}.#{pane_index}") {
        host.print(ctx, &format!("you are in {where_}"));
    }
    if !args.rest().is_empty() {
        host.print(ctx, &format!("extra arguments: {:?}", args.rest()));
    }
    // Options are how a plugin is configured, `@`-prefixed by convention —
    // `set -g @hello-greeting …` in .tmux.conf.
    if let Some(greeting) = host.get_option("@hello-greeting") {
        host.print(ctx, &greeting);
    }
    0
}

/// `#{plugin_hello_count}` — how many greetings this server has served.
fn hello_count(_host: &Host, _key: &str) -> Option<String> {
    Some(GREETED.load(Ordering::Relaxed).to_string())
}

/// Fired for every new session: record which one was last created, so the
/// effect is visible with `show-options -gv @hello-last-session`. A hook is
/// handed the empty [`Ctx`] — there is no client to print to — but it can
/// still `run` tmux commands, which queue globally.
fn on_session(host: &Host, _ctx: &Ctx, hook: &Hook) {
    if let Some(session) = &hook.session {
        host.set_option("@hello-last-session", session);
    }
}

declare_plugin! {
    name: "hello",
    version: "0.1.0",
    commands: {
        "hello-world" => {
            alias: "hw",
            template: "n:",
            usage: "[-n name] [arguments]",
            handler: hello,
        },
    },
    formats: { "plugin_hello_count" => hello_count },
    hooks: { "session-created" => on_session },
}
