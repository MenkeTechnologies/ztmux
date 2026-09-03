```text
███████╗████████╗███╗   ███╗██╗   ██╗██╗  ██╗
╚══███╔╝╚══██╔══╝████╗ ████║██║   ██║╚██╗██╔╝
  ███╔╝    ██║   ██╔████╔██║██║   ██║ ╚███╔╝
 ███╔╝     ██║   ██║╚██╔╝██║██║   ██║ ██╔██╗
███████╗   ██║   ██║ ╚═╝ ██║╚██████╔╝██╔╝ ██╗
╚══════╝   ╚═╝   ╚═╝     ╚═╝ ╚═════╝ ╚═╝  ╚═╝
```

[![CI](https://github.com/MenkeTechnologies/ztmux/actions/workflows/ci.yml/badge.svg)](https://github.com/MenkeTechnologies/ztmux/actions/workflows/ci.yml)
[![Docs](https://img.shields.io/badge/docs-online-blue.svg)](https://menketechnologies.github.io/ztmux/)
[![Port Report](https://img.shields.io/badge/port-report-8a2be2.svg)](https://menketechnologies.github.io/ztmux/port_report.html)
[![Parity vs tmux](https://img.shields.io/badge/parity%20vs%20tmux-1631%2F1631%20gated%20%2B%2012%20quarantined-brightgreen.svg)](parity/PARITY_ROADMAP.md)
[![Status](https://img.shields.io/badge/status-server%20%2B%20client%20running-brightgreen.svg)](https://menketechnologies.github.io/ztmux/)
[![Bug log](https://img.shields.io/badge/bug%20log-open%20gaps%20named-orange.svg)](docs/BUGS.md)
[![Reference](https://img.shields.io/badge/reference-tmux%203.x-00ffcc.svg)](https://github.com/tmux/tmux)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

### `[TMUX, REWRITTEN IN RUST — DONE RIGHT]`

> *"A 100%-functional tmux in Rust — the whole multiplexer,
> server and client, running."*
>
> *"Not a wrapper. Not control mode. The multiplexer itself."*
>
> *"Ported against the C, verified against the C — byte for byte, 1631/1631 gated parity cases passing."*

## `[FROM SOURCE, NOT FROM SCRATCH]`

**ztmux** is a from-source port of [tmux](https://github.com/tmux/tmux) to Rust — the whole
program: the server, the client, the grid/screen model, the input parser, layouts, the
command language, formats, and the terminal back end. It is **not** a wrapper around the
`tmux` binary and it is **not** control mode (`tmux -CC`); it is tmux, reimplemented. The
port stands on the upstream **tmux C sources**, vendored under
[`vendor/`](vendor/VENDOR.md) as a plain, read-only, SHA-pinned copy — the source of truth
every module is diffed against. Correctness is measured, not claimed — a
[parity suite](parity/PARITY_ROADMAP.md) runs identical inputs through the real `tmux` and
`ztmux` and diffs them byte-for-byte, and an anti-drift gate fails the build if a Rust
function is added whose name has no counterpart in the tmux C source.

### [`Docs`](https://menketechnologies.github.io/ztmux/) &middot; [`Port Report`](https://menketechnologies.github.io/ztmux/port_report.html) &middot; [`Parity`](parity/PARITY_ROADMAP.md) &middot; [`ztmux-core`](https://github.com/MenkeTechnologies/ztmux-core) &middot; [`tmux`](https://github.com/tmux/tmux)

---

## Table of Contents

- [\[0x00\] Overview](#0x00-overview)
- [\[0x01\] Install](#0x01-install)
- [\[0x02\] How the Port Is Built](#0x02-how-the-port-is-built)
- [\[0x03\] "Done Right"](#0x03-done-right)
- [\[0x04\] Parity vs System tmux](#0x04-parity-vs-system-tmux)
- [\[0x05\] Anti-Drift Gate — No Fake Functions](#0x05-anti-drift-gate--no-fake-functions)
- [\[0x06\] Layout](#0x06-layout)
- [\[0x07\] Porting Workflow](#0x07-porting-workflow)
- [\[0x08\] Extensions](#0x08-extensions)
- [\[0xFF\] License](#0xff-license)

---

## [0x00] OVERVIEW

A terminal multiplexer keeps your shells alive: split panes, detach and reattach, script
the whole thing. tmux is the reference implementation, ~30 years of C. ztmux ports that C
to Rust one subsystem at a time, holding behavior identical to upstream at every step. It
opens its own socket namespace (`ztmux-<uid>`) so it never collides with a running tmux.

**Status: 100% functional.** The port builds, runs, and self-hosts — `ztmux new-session`,
splits, detach/reattach, the command language, formats, and layouts all work — and the
parity suite is green at **1631/1631 gated (100%)** against the vendored tmux, with **12
cases quarantined** — they run and are diffed on every pass, but a divergence that only
appears on Linux keeps them out of the gate until it is root-caused
([`parity/quarantine.txt`](parity/quarantine.txt) says exactly which and why). Every
divergence the suite has ever caught has been root-caused and fixed or is recorded there;
surface that is still unported, or that no case can reach yet, is tracked openly in
[`docs/BUGS.md`](docs/BUGS.md) and [`parity/known_gaps/`](parity/known_gaps/) rather than
being counted as passing.

On top of the port, ztmux ships original subcommands with no tmux counterpart — live
dashboards and JSON-emitting inspectors for the running server (`ztmux --help`, and `[0x08]`).

> Distinct from [`ztmux-core`](https://github.com/MenkeTechnologies/ztmux-core), a native
> tmux *client* engine that speaks the wire protocol to an existing server for GUI hosts.
> **This** repo is the whole server + client. The two pair: ztmux and ztmux-core pin the
> identical `PROTOCOL_VERSION = 8` (`src/ported/tmux_protocol_h.rs` here,
> `src/transport.rs` there), so a GUI drives this server over the same wire protocol —
> and because both ends are MenkeTechnologies-owned, upstream tmux's release cadence can
> never break that contract.

---

## [0x01] INSTALL

Requires a Rust toolchain and a terminfo database (ncurses). No C libraries: the event
loop is Rust (`src/extensions/event_loop`, replacing tmux's libevent) and terminfo is read by
`terminfo-lean`, so there is nothing to `pkg-config`, no `-dev` package to install and no
Homebrew prefix to find.

```sh
cargo build --release
cargo run --release -- new-session       # start a server + session, like `tmux`
```

The binary is `ztmux`, and it links no C library beyond libc.

### Shadowing `tmux`

ztmux is the whole multiplexer, so it can stand in for `tmux` itself. Shadowing is opt-in,
and `ztmux shadow` is the whole opt-in:

```sh
ztmux shadow                  # install ~/.ztmux/{bin,man,completions}, print the shell lines
eval "$(ztmux shadow)"        # …apply them here, or paste them into ~/.zshrc
ztmux shadow -n               # print the lines without installing anything
```

It installs a `tmux` shim beside a `ztmux` one in `~/.ztmux/bin` (or a directory you name),
the man pages in `~/.ztmux/man` (`ztmux.1`, `ztmuxall.1`, and a `tmux.1` copy, so `man tmux`
reads this port's page), and the zsh completion in `~/.ztmux/completions` — `_ztmux`, plus a
`_tmux` wrapper that shadows the system one by file name, so the shimmed `tmux` completes
every tmux command *and* every ztmux extension. All of it is compiled into the binary, so
the install needs nothing from the source tree.

stdout is shell code only (the summary goes to stderr), so the same output both `eval`s and
pastes; a `PATH`/`MANPATH` line the environment already satisfies is printed commented out,
and `--all` prints every line uncommented. A real (non-symlink) file keeping a shim's name
is never clobbered, so `ztmux shadow /usr/local/bin` cannot replace an installed `tmux`.
Re-run it after a rebuild moves the binary, and `ztmux doctor` reports the install — PATH,
which `tmux` a command line actually reaches, MANPATH, and the completion — once it exists.

---

## [0x02] HOW THE PORT IS BUILT

One reference, vendored under [`vendor/`](vendor/VENDOR.md) as a plain committed copy
(the clone is self-contained and never depends on an upstream staying alive):

| Path | Upstream | Role |
| --- | --- | --- |
| `vendor/tmux/` | [tmux/tmux](https://github.com/tmux/tmux) (C) | **Source of truth.** Every ported module is diffed against its C counterpart. |
| `src/` | — | **The port.** The crate we own and evolve. Edit here. |

`Cargo.toml` declares its own `[workspace]` excluding `vendor/`, so Cargo never walks into
the reference. Every ported function carries a back-link comment to its C origin, e.g.:

```rust,ignore
// vendor/tmux/grid.c:320  grid_create()
pub fn grid_create(sx: u32, sy: u32, hlimit: u32) -> *mut grid {
```

---

## [0x03] "DONE RIGHT"

The port began as a faithful but almost-entirely-`unsafe` mechanical transpile. "Done
right" is turning that working skeleton into good Rust without ever drifting from tmux:

1. **Start from a working skeleton** — a running program to refactor, not a blank page.
2. **Shrink the `unsafe` surface** — replace raw-pointer intrusive lists and C-isms with
   safe Rust where behavior allows.
3. **Verify against C at every step** — a module isn't "ported" until it matches the C
   reference (see `[0x04]`).
4. **Keep it green** — `cargo build` and `cargo clippy` stay clean as code comes over.

---

## [0x04] PARITY VS SYSTEM tmux

ztmux is a port of tmux, so "correct" means **tmux itself**. The parity suite runs the same
inputs through the real `tmux` (reference) and `ztmux` (port) and compares byte-for-byte —
the same shape as the sibling ports ([zshrs](https://github.com/MenkeTechnologies/zshrs) vs
`zsh`, [strykelang](https://github.com/MenkeTechnologies/strykelang) vs `perl`).

```sh
bash parity/run_parity.sh --summary                       # ztmux vs tmux, every case
bash parity/verify_one.sh parity/cases/NAME.sh            # one case, in isolation (takes a PATH)
bash parity/run_known_gaps.sh                             # the inverted runner: "GAP" is the pass
```

The runner uses `target/release/ztmux` and builds it only when that file is **absent**, so
rebuild after changing the port or the run measures the previous binary.

Cases live in `parity/cases/` as tmux FORMAT strings (`#{e|+|:2,3}`) or shell scenarios.
It earns its keep: it root-caused a `#{l:…}` server crash to a dropped pointer increment in
`format_unescape`, fixed even-horizontal layout rounding and `#{pane_current_command}` on
macOS, regex backreferences, `#{!:}`, named buffers, loop variables, and the last layout
divergences — each pinned to a single case and then ported correctly. It now stands at
**1631/1631 gated cases passing (100%), byte-for-byte vs the vendored tmux**, plus **12
quarantined**: cases that pass on macOS and fail on the Linux CI runner, where the default
theme colours come out as terminal ANSI instead of RGB. Quarantined cases are still run and
still diffed on every pass — the count travels next to the percentage on the summary line
and in the JSON, and a failure still lands in the failure log; they simply do not gate while
the root cause is open ([`parity/quarantine.txt`](parity/quarantine.txt)). That is a
statement about what the suite measures, not a claim that nothing differs:
`parity/known_gaps/` holds behaviour that is deliberately still unported, and
`docs/BUGS.md` keeps an open list. 75 of the cases compare what an ATTACHED CLIENT
draws, by nesting a second server inside a pane of the first — the only way this suite can
see rendering at all. That technique is where most of the recent divergences were hiding:
copy-mode line numbers going stale on a cursor move, `display-menu -b` being parsed and
discarded, VS16 emoji measuring one column narrow, and OSC 8 hyperlinks being absent from
every build. See
[`parity/PARITY_ROADMAP.md`](parity/PARITY_ROADMAP.md) and the bug log
[`docs/BUGS.md`](docs/BUGS.md).

---

## [0x05] ANTI-DRIFT GATE — NO FAKE FUNCTIONS

A port can be faked by inventing Rust-only "helper" functions that don't exist in tmux,
inflating apparent completeness. `tests/ported_fn_names_match_c.rs` **fails the build** when
a free `fn` is added to `src/` whose name has no counterpart in `vendor/tmux`. Pre-existing
exceptions (libc wrappers, Rust glue) are frozen in
`tests/data/fake_fn_allowlist.txt` — an audit trail to burn down, not a free pass. The
[port report](https://menketechnologies.github.io/ztmux/port_report.html) tracks C→Rust
coverage per function.

Two more gates check the *shape* of the port rather than its names:
`tests/no_c_alloc_for_rust_types.rs` fails the build when a struct holding an owned Rust type
(`Vec`, `String`, `CString`, `Box`) is allocated with `xcalloc`, since all-zero bytes are not a
valid value for any of them; and `tests/no_key_code_truncation.rs` fails on any
`match key as u<N>`, which silently drops the high bits of a 64-bit `key_code` and once let a
mouse event alias an ASCII command letter. Alongside them, `tests/server_survives_bad_targets.rs`
drives the real binary on a private socket and asserts the server survives commands whose target
resolves to nothing, `tests/structured_output_under_c_locale.rs` pins the `-o json` path under a
non-UTF-8 client, and `tests/extension_flags_reach_the_verb.rs` pins that an extension's flags
arrive at the verb that implements it.

---

## [0x06] LAYOUT

```text
ztmux/
├── Cargo.toml         # the ztmux crate (own workspace root; excludes vendor/)
├── build.rs           # lalrpop (command grammar); no C libraries to link
├── src/               # THE PORT — edit here
│   └── extensions/    # original ztmux code (see [0x08]); not a port
│       ├── event_loop/  # the event loop in Rust (replaces libevent)
│       └── pkg/         # znative, the plugin manager (see docs/ZNATIVE.md)
├── plugin-abi/        # ztnative.rs — the native plugin ABI, one copied file
├── examples/          # eight installable plugins (native + TPM script)
├── completions/       # _ztmux zsh completion (generated by scripts/)
├── man/man1/          # ztmux.1 and ztmuxall.1 (installed by `ztmux shadow`)
├── parity/            # ztmux-vs-tmux byte-for-byte suite + roadmap
│   └── known_gaps/    # proven-unported behaviour, expected to diverge
├── regress/           # 32 of tmux's own regression scripts, run against ztmux
├── fuzz/              # cargo-fuzz target + differential fuzzing vs real tmux
├── scripts/           # gen_port_report.py, annotate_c_links.py
├── tests/             # anti-drift gate + allowlist
├── docs/              # GH Pages hub: index / report / port_report
├── vendor/
│   └── tmux/          # C source of truth  (read-only reference)
└── COPYING            # ISC (upstream notices)
```

---

## [0x07] PORTING WORKFLOW

1. Pick a subsystem (a `.rs` module under `src/`).
2. Open its C counterpart in `vendor/tmux/`.
3. Bring the Rust toward correct, idiomatic, memory-safe Rust — replacing the raw-pointer /
   `unsafe` C-isms with safe equivalents where behavior allows.
4. Keep it building (`cargo build`) and lint-clean (`cargo clippy`), and green against the
   parity suite (`bash parity/run_parity.sh`) at every step.

---

## [0x08] EXTENSIONS

Beyond the port, ztmux adds original subcommands with no upstream counterpart, under
[`src/extensions/`](src/extensions/). They live apart from the ported core — and are exempt
from the anti-drift gate (`[0x05]`) — precisely because they are *not* tmux. Each is either a
read-only query over the running server (built on the same structured `list-* -o json`
output) or a small mutating helper, and every one accepts `-o json` / `--json` for scripting.

They fall into a few families:

- **Inspection** — one-shot, pipeable views of the live server: process tables (`ps`,
  `pstree`, `mem`, `state`, `elapsed`), geometry (`size`, `density`, `layouts`, `solo`),
  directories and repositories (`cwd`, `project`, `git`, `remote`, `ahead`, `changes`,
  `stash`, `commit`, `conflicts`, `vcs`, `worktree`, `submodules`, `gone`), network (`ssh`,
  `net`, `ports`), clients (`who`, `readonly`, `idle`, `viewers`, `connected`, `constrain`,
  `keytable`, `control`, `utf8`), and configuration (`hooks`, `keys`, `monitor`, `remain`,
  `sync`, `limit`, `visual`, `mouse`, …).
- **Live TUIs** — `dashboard` (full-screen server monitor), `switcher` (fuzzy session/window/
  pane picker), `watch` (top-like per-pane process monitor).
- **Discovery** — `verbs [filter]` lists every verb ztmux answers to (ported commands, aliases,
  extensions, console builtins) with a one-line description, grouped by kind: `ztmux verbs pane`
  narrows to the pane-related ones, `-o json` emits `{verb, kind, description}` rows. It is built
  from the command table and the extension list themselves, and needs no server. `banner` prints
  the ztmux banner — the logo, the verb totals, and the socket's live session/window/pane/client
  counts, or that no server is running — which is also the console's opening screen.
- **Setup** — `shadow` installs the `~/.ztmux` shadow (a `tmux` shim, the man pages, the zsh
  completion) and prints the `PATH`/`MANPATH`/`fpath` lines that activate it, so
  `eval "$(ztmux shadow)"` makes `tmux` this port; `doctor` health-checks the build, the
  terminal, that install, the socket, the reachable server and the resource limits, exiting
  non-zero on warnings or errors so it drops into a CI gate. See `[0x01]`.
- **Console** — `repl` runs every line as `ztmux <line>` against the selected socket, with a
  reedline editor: Tab completes the command word (every command, alias, extension and builtin),
  a `-`-prefixed word against that verb's own flags, an option's fixed value set (`-o` →
  `json|jsonl|csv|tsv|table|yaml`), an extension's subcommand, and a shell builtin's path
  arguments against the filesystem. `verbs [filter]` lists every verb with its description,
  `banner` redraws the opening banner, and the shell builtins — `cd`, `pwd`, `dir`, `cat`,
  `echo`, `export`, `printenv`, `unset`, `mkdir`, `touch`, `rm`, `cp`, `mv`, `ln` — run in the
  console process, so the directory and environment they set are inherited by every line spawned
  afterwards (`cd ~/src/app` then `new-window` opens the window there). History persists to
  `~/.ztmux/repl_history`, and non-terminal stdin
  falls back to a plain line reader so `echo list-sessions | ztmux repl` stays scriptable. The
  editor is keyed emacs or vi, from the first source that names a mode: `$ZTMUX_REPL_EDIT_MODE`
  (`vi`/`emacs`, for a one-off console), `@ztmux-repl-edit-mode` (`set -g @ztmux-repl-edit-mode
  vi`), the server's `status-keys`, then `$VISUAL`/`$EDITOR` naming a vi editor — so a vi-keyed
  tmux config keys the console the same way without extra setup, and `help` prints the mode in
  use and where it came from. Reach it from inside a session with
  `bind : display-popup -E "ztmux repl"`.
- **Diagnostics** — every abnormal server exit leaves a `~/.ztmux/server-crash-<pid>.txt` or
  `server-panic-<pid>.txt` with the backtrace, written whether or not the server was started
  with `-v`, because the server that hits the bug is usually one that has been up for days.
  The same recorder logs each key-table destruction to `~/.ztmux/key-tables.log` with the
  caller's backtrace and the table's binding, default and reference counts — added to name the
  code path behind a server that lost every binding (`docs/BUGS.md`, Open).
- **Actions** — `prune`, `equalize`, `revive`, `clearall`, `retitle`, `bcast`, `layout`, and
  `pick` (batch sync/unmark/clear over a multi-pane mark set).
- **Automation** — `triggers` runs any ztmux command when a regex matches a pane's output
  (rules in `~/.ztmux/triggers.json`, armed with `ztmux triggers arm`), reviving tmux's removed
  `monitor-content` as a general sense→act loop. Add rules without touching the JSON via the
  inline wizard: `ztmux triggers wizard` (or `ztmux triggers add <name> <pane> <match> <action>`).
- **Plugins** — `znative` installs and loads tmux plugins from one
  content-addressed store: one `znative load owner/repo` line in `.tmux.conf`, self-installing
  on the first server start and zero-network on every one after. It loads ordinary **TPM script
  plugins** unmodified (a repo's `*.tmux` file, run with a `tmux` shim on `PATH` so it drives
  *this* server), and **native Rust plugins** — a `cdylib` the server `dlopen`s through the
  versioned [`ztnative`](plugin-abi/) C ABI — one copied file, no crate dependency —
  registering real tmux commands, `#{…}` format
  variables, and hook subscriptions, with no subprocess in the loop. tmux has never had a plugin
  ABI; this is one. The port always wins (a plugin cannot shadow a tmux command or format), and
  unloading purges every registration before the `dlclose`. See
  [`docs/ZNATIVE.md`](docs/ZNATIVE.md) and [`examples/`](examples/) — eight installable plugins,
  three of them native rewrites of TPM plugins (prefix-highlight, sensible, continuum).
- **Ratatui UI** (on by default) — original interactive surfaces rendered with
  ratatui rather than tmux's server draw: a which-key **hint bar** on the prefix, a floating
  **command palette** that completes the whole command line, not just the verb: every slot is
  read off the port's own data, so a command's flags come from the `args_parse` template it is
  validated against and each slot's meaning from its usage string. Tab offers command names and
  aliases, extension verbs and their subcommands, flags after `-` (and `--`), and then the value
  the slot wants — panes, windows, sessions and clients from the live server, paste buffers, key
  names and key tables, environment variables, options narrowed to the command's scope with their
  `on`/`off` or choice values, layouts, and filesystem paths. Also ratatui **clock**
  and **display-panes**,
  **edit-scrollback-in-`$EDITOR`** (`prefix e`), and **multi-pane selective sync** — mark panes
  (`prefix C-s`), sync the set (`prefix y`). Sync state is shown on the pane **border** — synced
  (red), selected (orange), trigger-armed (cyan) — which output can never overwrite.
  Opt into zellij-style **pane frames** with `@ztmux-zellij-mode on` (off by default): every pane
  is *inset* by a one-cell ring (like zellij, so a program can never draw on the frame) and gets a
  rounded box with its name in the top border; the box recolours for sync state. In this mode
  `prefix +` toggles a zellij-style **pane stack** — the focused pane fills the column, the rest
  collapse to one-row title bars (`ztmux stack` / `:stack`). A zellij-style **tab bar** of windows
  along the top (session badge, active tab highlighted) is a separate toggle — `ztmux tabs on` /
  `:tabs` — which restyles the status line and restores your prior status settings on `tabs off`.
  A zellij-style **session manager** (`ztmux sessions` / `:sessions`) opens a ratatui list of
  sessions: type to filter, Enter switches, `Ctrl-r` renames, `Ctrl-x` kills (with confirm),
  `Ctrl-n` makes a new one. `prefix C-f` toggles a **floating pane** — a real pane on a floating
  layout cell, drawn above the tiled layout, so it moves and resizes like any other pane
  (`move-pane -P centre`, `move-pane -X10 -Y4`, `resize-pane -x50% -y50%`); `prefix C-f` toggles
  focus between it and the pane you came from, creating one if none exists. `prefix *` opens one
  directly, and `prefix {` / `}` / `M-{` / `M-}` snap it to a corner at half size. With `mouse on`
  it is fully draggable: drag its **top border** to move it, any **other border or corner** to
  resize, and `Alt`-drag anywhere on it to move it. Panes underneath are clipped around it, so
  output from a tiled pane does not draw over the float, and a tiled border crossing it is
  drawn under it. A client redraw composites every visible cell — pane content, borders, pane
  status lines, floats — into a cached scene of spans first, then writes the spans out, so a
  float never costs a second pass over the terminal. Rendered screens are compared against the
  vendored tmux through a real VT emulator, not just the model state.
  Setting `@ztmux-float-autohide` (`set -wg @ztmux-float-autohide 1`, or the pane menu's
  `Auto-Hide Floating Panes`) switches to the zellij model instead: floating panes disappear
  while a tiled pane has focus and come back on `prefix C-f`. Off by default, so the upstream
  tmux behaviour of keeping the float on screen is what you get unless you ask.
  `ztmux modal on` (opt-in) installs zellij-style **modal keybindings**: `Ctrl-p` pane mode,
  `Ctrl-t` tab, `Ctrl-n` resize, `Ctrl-s` scroll, `Ctrl-o` session, `Ctrl-g` lock — each a sticky
  key table entered without a prefix; the hint bar (turned on automatically) shows the current
  mode's keys. `modal off` removes the entry keys and restores the prefix. Since the `Ctrl-*` keys
  are intercepted globally (the zellij trade-off), it is off by default.
  `ztmux resurrect save` / `ztmux resurrect restore` persist the whole server across restarts
  (zellij-style resurrectable sessions): `save` writes every session/window/pane — layout, cwd,
  running command and its full command line — to `~/.ztmux/resurrect/`, and `restore` recreates
  them (windows at their saved indexes, panes in their saved directories, exact tiled geometry,
  and floating panes back at their exact size and position). Restore works pane by pane: a live
  pane is left alone, a missing pane is split into its window, a missing window is added to its
  session, and only a missing session is created — so it fills gaps in a running server and is
  safe to repeat. Every pane that was running something is re-launched from its
  saved command line — a pane idling at a prompt saved none, so nothing is typed into it, and
  the shell itself is never re-run. `@ztmux-resurrect-processes` narrows that to a named list
  (`~name` matches anywhere in the command line) or turns it off with `false`. `resurrect
  list` shows saved snapshots. For continuum-style
  automatic persistence, `set -g @ztmux-resurrect-auto on`: the first client to attach spawns a
  detached daemon that re-saves every 15 minutes (pidfile-guarded, one per server); add
  `set -g @ztmux-resurrect-restore on` and it also restores the last snapshot once on a fresh
  server start.
  `ztmux open` / `:open` (also in the pane menu) scans the current pane for URLs and file paths
  and shows a ratatui picker — Enter opens the selection (a URL in `open`/`xdg-open`, a file in
  `$EDITOR` at its `file:line`, a directory revealed), `y` copies it (tmux buffer + OS clipboard).
  Like tmux-open / tmux-urlview, built in.
  Settings (all `set -g`):
  `@ztmux-ratatui off` disables the whole ratatui renderer for a classic plain-tmux server (on by
  default; takes effect on the next redraw); `@ztmux-hint on` shows the prefix hint bar (off by
  default);
  `@ztmux-zellij-mode on` enables the framed/inset mode (off by default; `@ztmux-pane-names` is a
  back-compat alias); `@ztmux-pane-name-format` overrides the frame name with a tmux format (e.g.
  `#{pane_index}: #{pane_current_command}`). With `@ztmux-ratatui off` the default draw path and
  the byte-for-byte parity suite are untouched.

Run `ztmux --help` for the current list, or `man ztmux` for the full reference — each
extension has its own entry under the EXTENSIONS section, and the zsh completion
([`completions/_ztmux`](completions/_ztmux)) describes every one inline.

---

## [0xFF] LICENSE

MIT — see [LICENSE](LICENSE). ztmux is a derivative work of tmux (Nicholas Marriott et al.),
ISC; the original notices are retained in [COPYING](COPYING) and under [`vendor/`](vendor/).
