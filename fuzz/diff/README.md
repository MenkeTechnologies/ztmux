# Differential fuzzing: ztmux vs tmux

These tests treat **real tmux as the oracle**: the same input is driven through
tmux and ztmux and their observable output is compared byte-for-byte. Any
difference is a parity bug in ztmux's port. This complements the in-process
`cargo fuzz` targets one level up (which only catch panics/asserts in isolated
Rust functions) — here the whole terminal is exercised end-to-end.

No Rust build step of its own; it drives the two binaries as subprocesses.

## Requirements

- `tmux` on `PATH` (or `$TMUX_BIN`) — the oracle.
- A built `ztmux` at `target/release/ztmux` / `target/debug/ztmux` (or `$ZTMUX_BIN`):

      cargo build --release

- Python 3.9+. No third-party packages.

## Modes

| mode    | what it fuzzes                          | how output is compared        |
|---------|-----------------------------------------|-------------------------------|
| `input` | the terminal parser / emulator          | `capture-pane -p` of the grid |
| `copy`  | copy-mode motion & selection (`send -X`)| the copied paste buffer       |

`input` generates random byte streams of interleaved printables, C0 controls
(tab/CR/LF/BS…), CSI/SGR/OSC/DCS sequences, and wide/combining UTF-8, then feeds
them to a pane. `copy` enters copy-mode over a fixed word/tab/whitespace-rich
buffer and runs a random sequence of no-argument copy-mode commands (cursor
movement, `select-word`/`select-line`, `rectangle-toggle`, …) before copying.

## Usage

Run from anywhere (paths are resolved relative to the repo):

    ./diff_fuzz.py copy  --count 200 --seed 1
    ./diff_fuzz.py input --count 200 --seed 1
    ./diff_fuzz.py both  --count 100

Each case is seeded, so a divergence is fully reproducible from its seed. On a
divergence the case is **minimised** (greedily shrunk while it still diverges),
printed, and saved to `repros/<mode>-NNN.json`. Replay one later:

    ./diff_fuzz.py repro repros/copy-001.json

Exit status is nonzero if any divergence was found, so it can gate CI.

Useful flags: `--width`/`--height` (pane size), `--settle` (seconds to wait for
a fresh server to render; raise it on a slow/loaded machine if `input` shows
spurious blank captures).

## Turning a divergence into a fix

1. Minimise (automatic) → note the tiny `case`.
2. Reproduce by hand: `printf` the bytes into a pane / drive the ops, and
   `capture-pane -p | od -c` against tmux.
3. Find the matching function in `vendor/tmux/` and diff the ztmux port.

## Status (as of v3.7.14)

Both modes are at parity under random fuzzing (0 divergences over hundreds of
`copy` sequences and `input` streams). One **pre-existing**, narrow parser gap
is known but not reliably hit by the random generator: a wide character (e.g. an
emoji) followed by backspace-and-overwrite renders differently from tmux (tmux
clears the half-overwritten cell to a space, ztmux keeps the wide glyph). It
predates this suite (reproduces on 3.7.11) and is unrelated to copy-mode.
