# ztnative — the ztmux native plugin ABI

**One file. Copy it into your plugin.**

`ztnative.rs` is the whole boundary between the ztmux server and a native
plugin: `#[repr(C)]` structs, `extern "C"` fn-pointer types, a version constant,
and the `declare_plugin!` macro that writes the entry point. It is **not a
crate** and there is nothing to depend on — the same file is compiled into the
host (`ztmux/src/lib.rs` includes it with `#[path]`) and into your plugin, so
both sides are guaranteed to agree on every layout without a dependency edge
between them. `ABI_VERSION` catches a plugin built against an older copy: the
host refuses to load it rather than reading a struct that has moved.

That is how a C plugin ABI has always worked — you copy the header — and it is
why a plugin builds with **zero dependencies**.

## Using it

```sh
curl -O https://raw.githubusercontent.com/MenkeTechnologies/ztmux/main/plugin-abi/ztnative.rs
mv ztnative.rs src/
```

```toml
# Cargo.toml — nothing to add but the crate type
[lib]
crate-type = ["cdylib"]
```

```rust
// src/lib.rs
mod ztnative;                       // the file you just copied
use crate::ztnative::{Args, Ctx, Host};
use std::os::raw::c_int;

fn hello(host: &Host, ctx: &Ctx, _args: &Args) -> c_int {
    host.print(ctx, "hello");
    0
}

declare_plugin! {                   // #[macro_export], so it is at your crate root
    name: "hello",
    version: "0.1.0",
    commands: {
        "hello-world" => { template: "", usage: "", handler: hello },
    },
}
```

```sh
ztmux znative add owner/tmux-hello   # clone, cargo build --release, dlopen
ztmux hello-world
```

The module must be named `ztnative` and live at your crate root: the macro
expands to `$crate::ztnative::…`, which is what lets one file serve a host and a
plugin that know nothing about each other.

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
| `format_expand` | expand `#{…}` against the running command's target — `#{S:…}` enumerates sessions, so this doubles as the read path |
| `register_command` / `register_format` / `register_hook` | dynamic registration (`declare_plugin!` calls these for you) |

Flags parsed out of the command line are read from the `Ctx`: `ctx.has('b')`,
`ctx.arg("t")`. Positional arguments arrive in `Args`.

## Rules the host enforces

- **The port always wins.** ztmux's own command table is scanned first, so a
  plugin can never shadow a tmux command; registering an existing name fails
  the load with a diagnostic. Formats work the same way.
- **Unload can never dangle.** Removing a plugin purges its registrations
  before the `dlclose`, and every dispatch looks the handler up by name at call
  time — a plugin command still sitting in a queued command list fails cleanly
  instead of jumping into an unmapped page.
- **Strings travel with their allocator.** What the host hands out comes back
  through `free_cstring`; a plugin's own strings are copied out through a
  callback rather than passed over as pointers.
- **`ABI_VERSION` is a hard gate.** A mismatched struct layout is undefined
  behaviour, so the host refuses the plugin rather than warning.

## Examples

Five installable plugins live in
[`examples/`](https://github.com/MenkeTechnologies/ztmux/tree/main/examples) —
`plugin-hello` (a command, a format and a hook), `plugin-battery` (status-line
state read in-process and cached), `plugin-sessionizer` (`run` to drive the
server, `format_expand` to read it back), `plugin-hooklog` (nine hook
subscriptions feeding a format), and `plugin-tpm-style` (an unmodified TPM
script plugin, for contrast). They reach this file with `#[path]` instead of
copying it, so they cannot drift from the original.

## Installing plugins

`znative`, ztmux's built-in plugin manager, installs both native plugins and
ordinary TPM script plugins from one content-addressed store. See
[docs/ZNATIVE.md](https://github.com/MenkeTechnologies/ztmux/blob/main/docs/ZNATIVE.md).

## License

MIT.
