# ztmux plugin examples

Five plugins, each complete and installable as-is, covering both plugin kinds
and every part of the [`ztnative`](../ztnative/) ABI. Install any of them into a
running server with a local path — no clone, no network:

```tmux
znative load path:examples/plugin-hello
```

| Example | Kind | Shows |
| --- | --- | --- |
| [`plugin-hello`](plugin-hello/) | native | The smallest complete plugin: one command (with an alias and a flag), one `#{…}` format, one hook. Start here. |
| [`plugin-battery`](plugin-battery/) | native | Formats as the point of the plugin — battery state in the status line, read in-process and cached, instead of `#(battery.sh)` forking on every status interval. Configured with `@` options. |
| [`plugin-sessionizer`](plugin-sessionizer/) | native | Driving the server: `run` queues tmux command text, `format_expand` reads state back (`#{S:…}` enumerates sessions), and flags are parsed by tmux's own `args_parse`. |
| [`plugin-hooklog`](plugin-hooklog/) | native | Hooks feeding a format: nine subscriptions append to a ring buffer that `#{plugin_hooklog_last}` reads on the next redraw. |
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
for p in hello battery sessionizer hooklog tpm-style; do
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

Copy `plugin-hello`, change the crate name, and depend on the published SDK
instead of the local path:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
ztnative = "0.1"
```

`declare_plugin!` writes the `ztnative_init` entry point and the trampolines;
everything else is ordinary Rust. See [`ztnative`](../ztnative/README.md) for
the ABI and [`docs/ZNATIVE.md`](../docs/ZNATIVE.md) for the plugin manager.
