# ztmux parity suite

ztmux is a from-source port of tmux, so the definition of "correct" is **tmux
itself** — specifically the exact tmux under `vendor/tmux` (currently `next-3.7`)
that `src/` is ported from. The suite runs the same inputs through that **vendored
tmux** (built from `vendor/tmux`, not the system's) and `ztmux`, and compares
byte-for-byte — mirroring the sibling ports (zshrs vs `zsh`, strykelang vs `perl`).

Version matters: layout rounding, div-by-zero formatting, and other format
details change between tmux releases, so comparing against a system tmux of a
different version (e.g. Ubuntu's 3.4) produces false diffs. The runner builds and
uses `vendor/tmux/tmux` by default; set `TMUX_REF=/path/to/tmux` to override.

## Running

```sh
# builds the vendored tmux reference + release ztmux if missing
bash parity/run_parity.sh                 # per-case OK/FAIL + totals
bash parity/run_parity.sh --summary       # totals only (CI)
bash parity/run_parity.sh --json parity/parity_summary.json
ZTMUX=target/debug/ztmux bash parity/run_parity.sh   # test a debug build
```

Failure detail (both outputs + unified diff, per case) lands in
`parity/parity_failures.log` (gitignored, truncated each run).

## Cases

`parity/cases/` holds two flavors:

- **`*.fmt`** — a single tmux **FORMAT** string (see FORMATS in `tmux(1)`). The
  runner expands it with `display-message -p` against a fresh detached session.
  This is the bulk of the suite: the format mini-language (arithmetic `#{e|…}`,
  comparisons `#{==:…}`, string ops `#{s/…}` / `#{=N:…}`, conditionals `#{?…}`,
  padding `#{p…}`, session/window/pane variables) is deterministic and stable
  across tmux versions, so it is the ideal parity surface.

    ```
    # parity/cases/010_arith_add.fmt
    #{e|+|:2,3}
    ```

- **`*.sh`** — a shell scenario for multi-command cases. `$TM` is exported as the
  binary already bound to a private socket; the script runs `$TM <cmd>` lines and
  prints deterministic output.

    ```sh
    # parity/cases/100_list_windows_after_neww.sh
    $TM new-window
    $TM list-windows -F '#{window_index}'
    ```

For every case the runner starts an **isolated server per binary** (`-L <uniq>`,
`-f /dev/null`, fixed 80×24 geometry), runs the case under a `timeout`, captures
stdout+stderr, kills the server, and compares.

### Determinism rules

Cases must not depend on host/time/version/pid/random state. Avoid `#{host}`,
`#{host_short}`, `#{version}`, `#{pid}`, `#{client_pid}`, wall-clock times, and
socket paths. The runner pins geometry (80×24), `LC_ALL=C`, and `-f /dev/null`
so width/height and option defaults are stable; still prefer computed formats
over version-sensitive option-default dumps (defaults drift between tmux
releases and the tmux version ztmux was ported from).

## Status

The port is seeded from a transpile, so — unlike a from-scratch rewrite — a large
part of the format engine already works. The suite's job is (a) to guard that
from regressing and (b) to grow coverage so the remaining gaps surface.

The suite has already paid off:

- `#{l:…}` (the literal operator) **crashed ztmux's server** — root-caused to a
  dropped pointer increment in `format_unescape`, fixed by a faithful re-port.
- **`405_select_layout`** — even-horizontal layout rounding was off by one column
  (`39|40` vs tmux's `40|39`). The port carried an *older* tmux algorithm that
  dumped the remainder on the last pane; ported the current C's leading-pane
  `remainder` distribution. Fixed. (This one was ALSO why comparing against a
  stale system tmux misled us — see the version note at the top.)

Currently known, on Linux the suite is green; the one remaining failure is
platform-specific:

- **`294_pane_cmd.fmt`** — `#{pane_current_command}` reports the server binary
  (`ztmux`) instead of the pane's process (`sleep`) **on macOS only**. Traced to
  pane spawn: the forked child enters the child branch but never reaches `execvp`
  (so the pane process stays as ztmux). Linux spawns/exec's correctly, so this
  case passes there. A macOS forkpty/child-exec bug, tracked here until fixed.

Failing cases stay in the suite (the CI job is advisory) until the underlying
code is ported correctly — removing them would fake a green suite.

## CI

The `Parity vs vendored tmux` job runs the suite advisory (`continue-on-error`,
not in the release-build gate) while the port's server surface is still coming
up — it surfaces the pass rate and uploads the failure log without failing the
pipeline, matching how `fmt`/`clippy` are handled here. Once the suite is
reliably green it should flip to a blocking ratchet (like strykelang's), so any
regression in ported behavior fails CI.

## Growing the suite

Add a `.fmt` (one format) or `.sh` (one scenario) file under `parity/cases/`.
Keep them small and single-purpose; number-prefix by category. The sibling
suites scaled this to thousands of cases — the same shape scales here.
