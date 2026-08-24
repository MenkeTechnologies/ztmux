# ztnative

**The native plugin ABI for [ztmux](https://github.com/MenkeTechnologies/ztmux) — tmux plugins written in Rust, loaded by the server.**

Every tmux plugin ever published is a shell script: a plugin manager clones a
repo, runs its `*.tmux` file, and that file drives the server by shelling out
to `tmux bind-key …`. There is no plugin ABI, because tmux has never had one.

`ztnative` is that ABI. A plugin is an ordinary `cdylib` the ztmux server
`dlopen`s, and what it registers are **real tmux commands**, **real `#{…}`
format variables**, and **real hook subscriptions** — resolved inside the
server, with no subprocess in the loop and no `tmux` binary involved.

Both sides of the boundary depend on this crate, so they agree on the exact
`#[repr(C)]` layout of the host table. Nothing about Rust's unstable
`repr(Rust)` layout, allocator, or panic ABI crosses it — only C-representable
data, behind a version gate the host refuses to load a mismatch on.

## A complete plugin

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
ztnative = "0.1"
```

```rust
use std::os::raw::c_int;
use ztnative::{declare_plugin, Args, Ctx, Hook, Host};

fn hello(host: &Host, ctx: &Ctx, _args: &Args) -> c_int {
    let who = ctx.arg("n").unwrap_or_else(|| "world".into());
    host.print(ctx, &format!("hello {who}"));
    0
}

fn count(_host: &Host, _key: &str) -> Option<String> {
    Some("42".into())
}

fn on_session(host: &Host, hook: &Hook) {
    if let Some(s) = &hook.session {
        host.set_option("@hello-last-session", s);
    }
}

declare_plugin! {
    name: "hello",
    version: "0.1.0",
    commands: {
        "hello-world" => { alias: "hw", template: "n:", usage: "[-n name]", handler: hello },
    },
    formats: { "plugin_hello_count" => count },
    hooks:   { "session-created" => on_session },
}
```

```sh
ztmux znative add owner/tmux-hello   # clone, cargo build --release, load
ztmux hello-world -n ztmux           # a tmux command like any other
ztmux display-message -p '#{plugin_hello_count}'
```

## What a plugin registers

| Section of `declare_plugin!` | Becomes |
| --- | --- |
| `commands:` | a tmux command — parsed by tmux's own argument parser (`template` is the flag template, e.g. `"bt:"`), dispatched from the command queue, usable from `.tmux.conf`, a key binding, the command prompt, and the CLI |
| `formats:` | a `#{…}` provider, consulted during format expansion |
| `hooks:` | a subscription called when a notification fires (`session-created`, `pane-exited`, `client-attached`, …) |

## What a plugin can call

| [`Host`] method | Purpose |
| --- | --- |
| `print` / `error` | write to the client that ran the command |
| `run` | parse and queue tmux command text — the whole command language |
| `get_option` / `set_option` | read and write options, including the `@user` options plugins configure themselves with |
| `format_expand` | expand `#{…}` against the running command's target |
| `register_command` / `register_format` / `register_hook` | dynamic registration (`declare_plugin!` calls these for you) |

Flags parsed out of the command line are read from the [`Ctx`]: `ctx.has('b')`,
`ctx.arg("t")`. Positional arguments arrive in [`Args`].

## Rules the host enforces

- **The port always wins.** ztmux's own command table is scanned first, so a
  plugin can never shadow a tmux command; registering an existing name fails
  the load with a diagnostic.
- **Unload can never dangle.** Removing a plugin purges its registrations
  before the `dlclose`, and every dispatch looks the handler up by name at call
  time — a plugin command still sitting in a queued command list fails cleanly
  instead of jumping into an unmapped page.
- **Strings travel with their allocator.** What the host hands out comes back
  through `free_cstring`; a plugin's own strings are copied out through a
  callback rather than passed over as pointers.
- **`ABI_VERSION` is a hard gate.** A mismatched struct layout is undefined
  behaviour, so the host refuses the plugin rather than warning.

## Installing plugins

`znative`, ztmux's built-in plugin manager, installs both native plugins and
ordinary TPM script plugins from one content-addressed store. See
[docs/ZNATIVE.md](https://github.com/MenkeTechnologies/ztmux/blob/main/docs/ZNATIVE.md).

## License

MIT.
