# znative — the ztmux plugin manager

`znative` is a built-in ztmux command for installing tmux plugins. It handles
both **script plugins** (the TPM kind — a repo with a `*.tmux` file that binds
keys and sets options, which is every tmux plugin published today) and
**native Rust plugins** (`cdylib`s loaded through the
[`ztnative`](../ztnative/) ABI — see [the SDK's README](../ztnative/README.md)).

It is **global only**: one content-addressed store under `$ZTMUX_HOME/pkg/`,
no per-project manifest or lockfile. The whole workflow is one line per plugin
in `.tmux.conf`:

```tmux
znative load owner/repo
```

On the first server start that installs the plugin and loads it; on every start
after, the same line loads it from the store with no network. There is no
separate install step, and no bootstrap clone to bless first — `znative` is
part of the server. It needs `git` on `PATH` for remote sources, and `cargo`
for native plugins that ship as source.

Because it is a real tmux command and not a CLI-only extension, the same line
works from `.tmux.conf`, the command prompt (`:znative list`), a key binding,
and the shell (`ztmux znative list`).

## Commands

| Command (aliases)            | Arguments   | What it does |
| ---------------------------- | ----------- | ------------ |
| `load` (`source`)            | `[NAME_or_SOURCE…]` | The one you need. With no argument, load every installed plugin. Given an installed **name** or a **source** already in the store, load it — **zero network**. Given a **source** not yet in the store (`owner/repo`, `github:…`, `git+URL`, `path:…`), install it first, then load. This is what a `.tmux.conf` calls. |
| `add` (`install`, `i`)       | `SOURCE…`   | Resolve, install into the store, record in the index, and load. (`load` self-installs, so this is mostly for installing without a `.tmux.conf` line.) |
| `remove` (`rm`, `uninstall`) | `NAME…`     | Unload (native), delete the store copy, drop the index row. |
| `list` (`ls`)                | —           | One line per installed plugin: `name  version  kind  source`, with `[loaded]` on the natives that are live. |
| `loaded`                     | —           | The native plugins mapped into *this* server right now, and the file each is running from. |
| `info` (`show`)              | `NAME`      | Full record: name, version, kind, source, store path, integrity, and — when loaded — the commands, formats and hooks it registered. |
| `update` (`upgrade`, `up`)   | `[NAME]`    | Re-resolve and reinstall from the recorded source (one, or all) — pulls the latest upstream and rebuilds. |
| `gc` (`-n`)                  | `[-n]`      | Remove `store/<name>@<version>/` directories not pinned by the index (orphans from old versions / upgrades) plus the `git/` clone cache. `-n` lists without deleting. |
| `clean`                      | —           | Clear the scratch directories (`git/`, `cache/`, `bin/`); the store and index are untouched. |
| `help`                       | —           | Usage. |

After an `update` installs a newer version, the previous
`store/<name>@<old>/` directory is left behind; `znative gc` reclaims it.

Errors print as `znative: <reason>` and the command fails.

## Sources

The `add`/`load` spec is auto-classified:

| Form                              | Example                                   | Resolves to |
| --------------------------------- | ----------------------------------------- | ----------- |
| `owner/repo`                      | `tmux-plugins/tmux-sensible`              | `git clone https://github.com/owner/repo` |
| `github:owner/repo`               | `github:tmux-plugins/tmux-resurrect`      | GitHub clone (explicit) |
| `git+URL`                         | `git+https://gitlab.com/team/plug.git`    | `git clone URL` |
| a URL ending `.git` or with `://` | `https://example.com/x.git`               | `git clone URL` |
| `path:DIR`                        | `path:examples/plugin-hello`              | local directory (no network) |
| an absolute / `./` / `../` / `~` path | `~/src/my-plugin`                     | local directory (no network) |

**Install by version** — any remote form may carry an `@ref` suffix (split
after the last `/`) to pin a tag, branch, or commit:
`owner/repo@v1.2.0`, `git+https://host/x.git@main`. The pin is **recorded** in
the index (`source = github:owner/repo@v1.2.0`), so `list` shows it, `update`
re-fetches that exact ref (not HEAD), and `load owner/repo@v1.2.0` matches only
that pin. Clones are shallow (`git clone --depth 1 [--branch REF]`); a commit
sha a shallow `--branch` clone cannot reach falls back to a full clone plus
`git checkout`.

## Plugin kinds

| Kind       | Loaded by                                   | Built with |
| ---------- | ------------------------------------------- | ---------- |
| **native** | `dlopen` + the `ztnative` ABI               | `cargo build --release` when no prebuilt `lib*.{dylib,so}` is present |
| **script** | running its `*.tmux` files (`run-shell -b`) | nothing — run as-is, exactly like TPM |

When there is no explicit `ztnative.toml`, the kind is auto-detected:

1. a prebuilt `lib*.{dylib,so}` at the repo root, **or** a `Cargo.toml`
   mentioning `cdylib` → **native**;
2. otherwise any `*.tmux` file → **script**;
3. otherwise `znative` reports it cannot determine the kind.

An unmodified TPM plugin therefore installs with no metadata at all.

### How script plugins reach the server

A tmux plugin drives the server by shelling out to `tmux`. `znative` runs each
`*.tmux` file with a `tmux` shim first on `PATH` — a two-line script in
`$ZTMUX_HOME/pkg/bin/` that execs *this* ztmux against *this* server's socket.
Without it, a plugin's `tmux bind-key …` would reach whatever `tmux` binary the
machine happens to have (or none), and on a `-S`/`-L` server it would configure
the wrong server entirely: ztmux deliberately adopts `$TMUX` only for a socket
in its own directory, so that a ztmux command run inside a *real* tmux pane does
not speak ztmux's protocol at a tmux server.

Files named `*.tmux` are made executable when they are copied into the store,
so a repo that shipped one without the bit still loads.

### What a native plugin can do

A native plugin is a `cdylib` compiled against [`ztnative`](../ztnative/). It
registers, through a versioned C ABI:

- **tmux commands** — a real `cmd_entry`, parsed by tmux's own argument parser
  and dispatched from the command queue, so it works from `.tmux.conf`, a key
  binding, the command prompt, and the CLI;
- **`#{…}` formats** — a provider consulted during format expansion, so a
  plugin can extend the status line without a shell job on every redraw;
- **hooks** — a subscription called when a notification fires
  (`session-created`, `pane-exited`, `client-attached`, …).

and calls back into the server for: printing to the client, running tmux
command text, reading and writing options (including the `@user` options
plugins configure themselves with), and expanding formats against the running
command's target.

Two rules the host enforces:

- **The port always wins.** `cmd_find` scans tmux's own command table first, so
  a plugin can never shadow a tmux command; registering a name that already
  exists fails the load with a diagnostic. Formats work the same way.
- **Unload can never dangle.** Removing a plugin purges its registrations
  before the `dlclose`, and every dispatch looks its handler up by name at call
  time — a plugin command still sitting in a queued command list after its
  plugin was removed fails cleanly instead of jumping into an unmapped page.

## The store

Everything lives under `$ZTMUX_HOME/pkg/` (`$ZTMUX_HOME` defaults to
`~/.ztmux`, the directory ztmux already keeps its logs in):

```text
$ZTMUX_HOME/pkg/
  store/<name>@<version>/   # the installed plugin (content-addressed)
  installed.toml            # the global index — the source of truth
  git/                      # scratch: remote clones land here, then copy to store/
  bin/                      # the `tmux` shim script plugins run with
  cache/                    # internal scratch
```

The copy into `store/` excludes `.git/` and `target/`, so the store holds only
loadable content. Each install is SHA-256 pinned as `sha256-<hex>` in
`installed.toml`. A record looks like:

```toml
[[package]]
name = "resurrect"
version = "0.1.0"
source = "github:tmux-plugins/tmux-resurrect"
kind = "script"
integrity = "sha256-…"
run = ["resurrect.tmux"]        # script: the *.tmux files to run
# native plugins record instead:
# lib = "libbattery.dylib"      # the cdylib to dlopen
```

A native plugin's **name and version come from the compiled artifact** when the
repo declares none: the plugin's own `PluginInfo` is read before it is
installed, so a repository called `tmux-battery` whose plugin declares itself
`battery` installs as `battery@0.2.0` and `znative info battery` finds it.

## `ztnative.toml` (optional manifest)

A plugin repo may ship a `ztnative.toml` at its root to declare metadata and
the load recipe explicitly (it overrides auto-detection):

```toml
[plugin]
name = "battery"
version = "0.1.0"
description = "battery status for the status line"

# Native (Rust cdylib) plugin:
[native]
lib = "battery"          # produces lib<lib>.{dylib,so}
# build = true           # run `cargo build --release`; defaults to true
                         # when a Cargo.toml is present

# …or a script plugin:
# [script]
# run = ["battery.tmux"]  # files to execute, in order
```

Standard TPM repos need no `ztnative.toml` at all.

## In your `.tmux.conf`

List the plugins you want with `znative load owner/repo`, one per line, in load
order. First start installs each; later starts load from the store with no
network:

```tmux
znative load tmux-plugins/tmux-sensible
znative load tmux-plugins/tmux-resurrect
znative load path:~/src/my-native-plugin
```

A bare `znative load` (no argument) loads everything already in the store —
useful if you prefer to `znative add` interactively and keep one line in the
config.

There is no counterpart to TPM's `prefix + I` / `prefix + U`: installing is
`znative add`, updating is `znative update`, and both are ordinary commands you
can bind if you want them on a key.

## Migrating from TPM

| TPM | znative |
| --- | --- |
| `set -g @plugin 'owner/repo'` + `run '~/.tmux/plugins/tpm/tpm'` | `znative load owner/repo` |
| `prefix + I` (install) | happens on the next server start, or `znative add owner/repo` |
| `prefix + U` (update) | `znative update [NAME]` |
| `prefix + alt-u` (clean) | `znative gc` |
| `~/.tmux/plugins/` | `$ZTMUX_HOME/pkg/store/<name>@<version>/` |
| clone TPM first, bless it in the config | nothing — `znative` is part of the server |

Plugin repos themselves need no changes.

## Examples

```tmux
# In .tmux.conf — self-installing on first use, zero-network after.
znative load tmux-plugins/tmux-sensible          # script: sane defaults
znative load tmux-plugins/tmux-resurrect         # script: save/restore
znative load tmux-plugins/tmux-yank@v2.3.0       # pinned ref
znative load path:examples/plugin-hello          # local checkout (native)
znative load git+https://gitlab.com/team/p.git   # non-GitHub URL
```

```sh
# From the shell, against a running server.
ztmux znative list
ztmux znative info resurrect
ztmux znative update
ztmux znative gc -n
```

## Writing a native plugin

See [`ztnative`](../ztnative/README.md) for the ABI and
[`examples/`](../examples/) for five installable plugins:

| Example | Kind | Shows |
| --- | --- | --- |
| `plugin-hello` | native | the smallest complete plugin — one command, one format, one hook |
| `plugin-battery` | native | formats as the point: status-line state read in-process and cached, not `#(script.sh)` per interval |
| `plugin-sessionizer` | native | `run` to drive the server, `format_expand` (`#{S:…}`) to read it back |
| `plugin-hooklog` | native | nine hook subscriptions feeding a `#{…}` variable |
| `plugin-tpm-style` | script | a TPM plugin installed unmodified |

Install any of them straight from the tree:

```tmux
znative load path:examples/plugin-battery
```

Building from a `path:` source runs `cargo build --release` and copies the
artifact into the store; the plugin's own tree is never written to, and a
`Cargo.toml` always wins over a `lib*.{dylib,so}` left lying next to it, so an
install can never pick up a build older than the source.
