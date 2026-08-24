# ztmux plugin examples

Eight plugins, each complete and installable as-is, covering both plugin kinds
and every part of the [`ztnative`](../plugin-abi/) ABI. Three of them are native
rewrites of TPM plugins, so the two models can be compared line for line.
Install any of them into a running server with a local path — no clone, no
network:

```tmux
znative load path:examples/plugin-hello
```

| Example | Kind | Shows |
| --- | --- | --- |
| [`plugin-hello`](plugin-hello/) | native | The smallest complete plugin: one command (with an alias and a flag), one `#{…}` format, one hook. Start here. |
| [`plugin-battery`](plugin-battery/) | native | Formats as the point of the plugin — battery state in the status line, read in-process and cached, instead of `#(battery.sh)` forking on every status interval. Configured with `@` options. |
| [`plugin-sessionizer`](plugin-sessionizer/) | native | Driving the server: `run` queues tmux command text, `format_expand` reads state back (`#{S:…}` enumerates sessions), and flags are parsed by tmux's own `args_parse`. |
| [`plugin-hooklog`](plugin-hooklog/) | native | Hooks feeding a format: nine subscriptions append to a ring buffer that `#{plugin_hooklog_last}` reads on the next redraw. |
| [`plugin-prefix-highlight`](plugin-prefix-highlight/) | native | **tmux-prefix-highlight, natively.** The original rewrites `status-right` at load into a baked `#{?client_prefix,…}` string; this resolves per redraw from the live client, reading `#{client_prefix}` / `#{pane_in_mode}` / `#{synchronize-panes}` through the format context. Same `@prefix_highlight_*` options. |
| [`plugin-sensible`](plugin-sensible/) | native | **tmux-sensible, natively.** The original forks ~30 processes per server start to read and set options; this uses two in-process calls each, and keeps the rule that matters — never clobber a value the user chose. Shows `on_load:`. |
| [`plugin-continuum`](plugin-continuum/) | native | **tmux-continuum, natively.** The original appends `#(continuum_status.sh)` to `status-right`, forking a process on every redraw to check the clock; this treats the hooks that make a snapshot stale as the trigger, debounced by an interval. |
| [`plugin-tpm-style`](plugin-tpm-style/) | script | A TPM plugin — a `*.tmux` file, no Rust and no manifest — installed unmodified. |

## The two kinds

A **script** plugin is what the tmux ecosystem already ships: a repository with
a `*.tmux` file that drives the server by running `tmux …`. `znative` installs
those with no changes, so `znative load tmux-plugins/tmux-resurrect` works today.

A **native** plugin is a Rust `cdylib` the server `dlopen`s. What it registers
are the host's own primitives:

- a **command** — a real entry in tmux's command table, so tmux parses its
  flags and it works from `.tmux.conf`, a key binding, the command prompt and
  the CLI alike;
- a **`#{…}` format** — consulted during expansion, so the status line can show
  plugin state with no shell job on the redraw path;
- a **hook** — called when a notification fires.

and it calls back for `print`/`error`, `run` (any tmux command text),
`get_option`/`set_option`, and `format_expand`.

## Trying them out

```sh
# In a scratch server, so nothing touches your own config or plugin store.
ZTMUX_HOME=/tmp/ztnative-demo ztmux -f /dev/null -L demo new-session -d
for p in hello battery sessionizer hooklog prefix-highlight sensible continuum tpm-style; do
    ZTMUX_HOME=/tmp/ztnative-demo ztmux -L demo znative add "path:examples/plugin-$p"
done

ZTMUX_HOME=/tmp/ztnative-demo ztmux -L demo znative list
ZTMUX_HOME=/tmp/ztnative-demo ztmux -L demo hello-world -n you
ZTMUX_HOME=/tmp/ztnative-demo ztmux -L demo display-message -p '#{plugin_battery}'
ZTMUX_HOME=/tmp/ztnative-demo ztmux -L demo hooklog
ZTMUX_HOME=/tmp/ztnative-demo ztmux -L demo kill-server
```

Installing from a `path:` source builds the plugin with `cargo build --release`
and copies the result into the store; the source tree is left alone.

## Writing your own

Copy `plugin-hello`, change the crate name, and copy
[`plugin-abi/ztnative.rs`](../plugin-abi/ztnative.rs) into your `src/` as
`mod ztnative;` instead of pointing `#[path]` at the in-tree original. A plugin
has no dependencies at all:

```toml
[lib]
crate-type = ["cdylib"]
```

`declare_plugin!` writes the `ztnative_init` entry point and the trampolines;
everything else is ordinary Rust. See [`ztnative`](../plugin-abi/README.md) for
the ABI and [`docs/ZNATIVE.md`](../docs/ZNATIVE.md) for the plugin manager.
