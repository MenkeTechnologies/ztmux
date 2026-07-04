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
[![Parity vs tmux](https://img.shields.io/badge/parity%20vs%20tmux-1080%2F1080%20(100%25)-brightgreen.svg)](parity/PARITY_ROADMAP.md)
[![Status](https://img.shields.io/badge/status-100%25%20functional-brightgreen.svg)](docs/BUGS.md)
[![Reference](https://img.shields.io/badge/reference-tmux%203.x-00ffcc.svg)](https://github.com/tmux/tmux)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

### `[TMUX, REWRITTEN IN RUST — DONE RIGHT]`

> *"The world's first 100%-functional tmux in Rust — the whole multiplexer,
> server and client, running."*
>
> *"Not a wrapper. Not control mode. The multiplexer itself."*
>
> *"Ported against the C, verified against the C — byte for byte, 1080/1080 parity cases passing."*

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
parity suite is green at **1080/1080 (100%)** against the vendored tmux. Every bug the harness
found has been root-caused and fixed (see [`docs/BUGS.md`](docs/BUGS.md)).

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

Requires a C `libevent` (tmux's event-loop library) and a terminfo database (ncurses),
exactly like tmux.

```sh
# macOS
brew install libevent ncurses
cargo build --release
cargo run --release -- new-session       # start a server + session, like `tmux`

# Debian / Ubuntu
sudo apt-get install libncurses-dev libevent-dev
cargo build --release
```

The binary is `ztmux`. On macOS the build links Homebrew's `libevent` automatically; set
`TMUX_RS_DISABLE_HOMEBREW_LIBS=1` to skip the Homebrew search path. Linking can be forced
with the `static` / `dynamic` features.

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
bash parity/run_parity.sh --summary       # ztmux vs tmux, every case
```

Cases live in `parity/cases/` as tmux FORMAT strings (`#{e|+|:2,3}`) or shell scenarios.
It earns its keep: it root-caused a `#{l:…}` server crash to a dropped pointer increment in
`format_unescape`, fixed even-horizontal layout rounding and `#{pane_current_command}` on
macOS, regex backreferences, `#{!:}`, named buffers, loop variables, and the last layout
divergences — each pinned to a single case and then ported correctly. It now stands at
**1080/1080 cases passing (100%), byte-for-byte vs the vendored tmux, with zero known
divergences.** See [`parity/PARITY_ROADMAP.md`](parity/PARITY_ROADMAP.md) and the bug log
[`docs/BUGS.md`](docs/BUGS.md).

---

## [0x05] ANTI-DRIFT GATE — NO FAKE FUNCTIONS

A port can be faked by inventing Rust-only "helper" functions that don't exist in tmux,
inflating apparent completeness. `tests/ported_fn_names_match_c.rs` **fails the build** when
a free `fn` is added to `src/` whose name has no counterpart in `vendor/tmux`. Pre-existing
exceptions (libc/libevent wrappers, Rust glue) are frozen in
`tests/data/fake_fn_allowlist.txt` — an audit trail to burn down, not a free pass. The
[port report](https://menketechnologies.github.io/ztmux/port_report.html) tracks C→Rust
coverage per function.

---

## [0x06] LAYOUT

```text
ztmux/
├── Cargo.toml         # the ztmux crate (own workspace root; excludes vendor/)
├── build.rs           # lalrpop (command grammar) + libevent linking
├── src/               # THE PORT — edit here
│   └── extensions/    # original ztmux subcommands (see [0x08]); not a port
├── completions/       # _ztmux zsh completion (generated by scripts/)
├── parity/            # ztmux-vs-tmux byte-for-byte suite + roadmap
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
- **Live TUIs** — `dashboard` (full-screen server monitor), `switch` (fuzzy session/window/
  pane picker), `watch` (top-like per-pane process monitor).
- **Actions** — `prune`, `equalize`, `respawn`, `clear`, `retitle`, `bcast`, `layout`.
- **Automation** — `triggers` runs any ztmux command when a regex matches a pane's output
  (rules in `~/.ztmux/triggers.json`, armed with `ztmux triggers arm`), reviving tmux's removed
  `monitor-content` as a general sense→act loop.

Run `ztmux --help` for the current list, or `man ztmux` for the full reference — each
extension has its own entry under the EXTENSIONS section, and the zsh completion
([`completions/_ztmux`](completions/_ztmux)) describes every one inline.

---

## [0xFF] LICENSE

MIT — see [LICENSE](LICENSE). ztmux is a derivative work of tmux (Nicholas Marriott et al.),
ISC; the original notices are retained in [COPYING](COPYING) and under [`vendor/`](vendor/).
