```
     _                        
 ___| |_ _ __ ___  _   ___  __
|_  / __| '_ ` _ \| | | \ \/ /
 / /| |_| | | | | | |_| |>  < 
/___|\__|_| |_| |_|\__,_/_/\_\
```

# ztmux

**A Rust port of tmux.**

ztmux is a from-source port of [tmux](https://github.com/tmux/tmux) to Rust. It is not a
wrapper around the `tmux` binary and not control mode — it is the tmux program itself,
reimplemented in Rust: the server, the client, the grid/screen model, the input parser,
layouts, the command language, formats, and the terminal back end.

> Note: `ztmux` (this repo, the full program) is distinct from
> [`ztmux-core`](../ztmux-core), a native tmux *client* engine that speaks tmux's wire
> protocol to an existing server for GUI hosts. This repo ports the whole server+client.

## How this port is structured

We stand on two references, both vendored under [`vendor/`](vendor/VENDOR.md) as plain
committed copies (so the repo is self-contained):

- **`vendor/tmux/`** — the upstream tmux **C sources**, our source of truth. Every ported
  module is validated against its C counterpart here.
- **`vendor/tmux-rs/`** — [richardscollin/tmux-rs](https://github.com/richardscollin/tmux-rs),
  a substantially complete Rust port (ISC). We **started from this**: the crate at the repo
  root was seeded from `vendor/tmux-rs/src`, then renamed to `ztmux` and taken over as our
  own living code.

The pristine `vendor/tmux-rs` copy stays untouched so that

```sh
git diff --no-index vendor/tmux-rs/src src
```

shows exactly what has diverged since the fork, and upstream fixes can be cherry-picked in.

## Layout

```
ztmux/
├── Cargo.toml         # the ztmux crate (own workspace root; excludes vendor/)
├── build.rs           # lalrpop (command grammar) + libevent linking
├── src/               # THE PORT — edit here
├── vendor/
│   ├── tmux/          # C source of truth  (read-only reference)
│   └── tmux-rs/       # Rust head start    (read-only reference)
└── COPYING            # ISC
```

## Building

Requires a C `libevent` (tmux's event loop library). On macOS it is linked statically from
Homebrew by default:

```sh
brew install libevent          # macOS
cargo build
cargo run -- new-session       # start a server + session, like `tmux`
```

Linking can be forced with the `static` / `dynamic` features; set
`TMUX_RS_DISABLE_HOMEBREW_LIBS=1` to skip the Homebrew search path on macOS.

## Porting workflow

1. Pick a subsystem (a `.rs` module under `src/`).
2. Open its C counterpart in `vendor/tmux/` and the seeded version in `vendor/tmux-rs/`.
3. Bring the Rust toward correct, idiomatic, memory-safe Rust — replacing raw-pointer /
   `unsafe` C-isms carried over by the seed with safe equivalents where behavior allows.
4. Keep it building (`cargo build`) and lint-clean (`cargo clippy`) at each step.

## License

MIT — see [LICENSE](LICENSE). ztmux is a derivative of tmux and tmux-rs (both
ISC); their original notices are retained in [COPYING](COPYING) and under
`vendor/`.
