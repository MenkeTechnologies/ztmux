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

**1173/1173 cases pass (100%) vs the vendored tmux — zero known divergences.** The
suite grew from 122 → 380 → 646 → 661 → 665 → 675 → 680 → 684 → 686 → 689 → 774 → 840 → 900 → 1080 → 1107 → 1115 → 1121 → 1123 → 1130 → 1134 → 1166 → 1173 cases.
The 1211–1390 block (fanned out across format / options / window-pane-layout /
buffer-session authors) surfaced and fixed two real bugs: `split-window -f`
(full-size split with a pre-existing split) crashed the server on a u32 underflow
in `layout_resize_child_cells` — C wraps `u_int`, now `wrapping_sub` (layout.c);
and `new-window`'s usage string had dropped the `[argument ...]` token. Both are
now pinned by cases 1319 and 1389. See `parity/verify_one.sh` for the single-case
verifier used to author the block. The second expansion (blocks
800–1069) deepened the areas the first round found bugs in, and surfaced two more
gaps plus a cluster of layout divergences; the final round closed the layout
cluster and pushed the suite fully green. The latest block (1000–1084) adds
format-engine edge cases (trailing/escaped `#`, `=N` truncation, `p` padding,
`s///` substitution + backrefs, `!`/`!!`/`==`/`!=`/`||`/`&&`, `e|op|` arithmetic,
`m`/`l`/`q`/`b`/`d` modifiers, nested modifiers) and command-level scenarios
(option get/set/unset across scopes, window create/rename/move/swap/kill/renumber,
buffer set/list/rename/delete, pane split/index) — all byte-identical to upstream.
The 1085–1150 block broadens further: pane-border-status as a window option,
next/previous/last-window navigation with wraparound, respawn-pane, kill/swap/
break/rotate-pane, select-layout, resize-pane, set/show/unset-environment,
new/kill-session, synchronize-panes, status options, and more `e|op|`/`s///`/
`m:`/nested-modifier format cases — all byte-identical to upstream.
The 1151–1210 block adds deterministic state-variable formats
(`window_index`/`name`, `session_name`, `pane_index`, `window_panes`,
`window_active`, `window_zoomed_flag`, `pane_in_mode`, `window_width`/`height`,
`window_layout`, and conditionals/arithmetic/substitution over them) plus more
commands: hooks (set/show), environment scopes, buffer append/auto-name,
link/unlink/join/break/swap/kill-pane and -window, clear-history, last-pane,
window-size/resize-window, select-layout (tiled/main-vertical), next-layout,
status-position/justify, prefix, remain-on-exit/allow-rename/automatic-rename.

Round-8 fixes:

The 1439–1470 block was written against the areas the last few bug rounds came
out of rather than against new commands: the copy-mode command table and its
formats (3 cases before this block, against 91 table entries), the grid as seen
through `capture-pane` (2 cases before this block), the signed offset arithmetic
in layout/resize, and the popup/menu argument parsers. It found five bugs, two
of them server crashes:

- **`#{selection_mode}` and `#{search_timed_out}` never expanded** (1443, 1447)
  — `window_copy_formats` (`window-copy.c:1139`, `:1152`) dropped the `selflag`
  switch and the timeout entry, so both expanded empty where tmux prints
  `char`/`word`/`line` and `0`/`1`.
- **a failed search kept the previous search's match count** (1447) —
  `window_copy_clear_marks` (`window-copy.c:4805`) resets `searchcount = -1` and
  `searchmore = 0`; the port only freed the mark array, so after a search that
  matched nothing `#{search_count}` still reported the earlier count instead of
  expanding empty.
- **`append-selection` took the server down** (1448) — `paste_set` notified with
  the caller's `name`, and `window_copy_append_selection` borrows that name out
  of the very buffer being replaced (via `paste_get_top`, which in C returns an
  `xstrdup`). By the time the notify ran, `paste_free(old)` had dropped the
  string. C notifies with `pb->name`, the copy `paste_set` just made; now so
  does the port.
- **`capture-pane -S -5` took the server down** (1454) — `gd->hsize + n` with a
  negative `int` is an unsigned wrap in C and an overflow panic in a Rust debug
  build. Now `wrapping_add_signed` on both the `-S` and `-E` paths, the same
  shape as the earlier `tty_cursor` and `previous-prompt` fixes.
- **percentages over 100% were rejected** (1462) — `args_string_percentage` and
  `args_string_percentage_and_expand` bounded the numerator at 100 where the C
  bounds it at 1000 (`arguments.c:1013`, `:1081`), so `resize-pane -x 150%`
  failed with "width too large" instead of resolving to 120 and letting the
  layout clamp it to the window. A unit test had encoded the wrong bound and was
  corrected against the C.

Two gaps the block proved are unported rather than wrong went to
`parity/known_gaps/` and were then ported (Round-9 below), so both files are gone
again and their behaviour is covered by cases 1471–1477.

Round-9 port:

Closing those two gaps. The copy-mode command table gained the 10 entries
next-3.7 has and this port did not — `refresh-{on,off,toggle}`,
`scroll-exit-{on,off,toggle}`, `recentre-top-bottom`,
`cursor-centre-{vertical,horizontal}` and `selection-mode` — and lost
`refresh-from-pane`, which upstream replaced with an automatic refresh. That
refresh is the substantial half: `window_copy_sync_snapshot`/`sync_backing`/
`do_refresh`/`refresh_arm`/`refresh_timer`/`refresh_start`/`refresh_stop`, which
reconcile the backing screen incrementally against the live pane using the grid's
monotonic scroll counters (`scroll_added`/`scroll_collected`/`scroll_generation`,
`tmux.h:898`, maintained at `grid.c:470`, `:508`, `:521`, `:559`, `:1611`). Along
the way `grid_collect_history` gained its `all` parameter and the caller that uses
it, `session_update_history` (`session.c:765`), so a changed `history-limit` now
collects the history that no longer fits.

`capture-pane` gained `-F`, `-H`, `-L` and `-M`, which needed the
`GRID_LINE_HYPERLINK` line flag (`tmux.h:804`) and the `get_screen` callback on
`struct window_mode` (`tmux.h:1180`) as substrate.

Porting `recentre-top-bottom` surfaced one more crash of the family this suite
keeps finding: the C adjusts the cursor row by the signed change in the scroll
offset as wrapping `u_int` arithmetic, which panics as a plain Rust add on the
common case of recentring a scrolled-back view. Fixed with explicit wrapping ops
and pinned by case 1471.

`scroll-to-mouse` is the one command that did not graduate: it drags the scrollbar
slider (`wp->sb_slider_h`, `tty.mouse_slider_mpos`), so it belongs to the unported
pane-scrollbars subsystem and is recorded there.

Round-7 fix:

- **`switch-client -O`** (1111–1113) — the `-O order` flag was unrecognized
  (`unknown flag -O`). Ported it faithfully: added `O:` to the arg spec + usage
  (`c:EFlnO:pt:rT:Z`, `… [-O order]`) and built the `sort_criteria` in the exec
  (`cmd-switch-client.c:109`), erroring `invalid sort order` on a bad `-O`.
  This exposed that ztmux's `session_next_session`/`session_previous_session`
  (`session.c:277`/`:300`) had a stale signature — they took no `sort_crit` and
  had a non-C `s2 == s → NULL` shortcut. Re-ported both against the C to sort
  via `sort_get_sessions(sort_crit)` and index with wraparound (the
  same-session case is handled downstream by `server_fn`'s `s_new == s`), and
  threaded `sort_crit` through the `server_fn` destroy callers (C passes `NULL`
  → a `SORT_END` criteria = keep RB name order).

Round-6 fix:

- **`display-message -C`** (1109–1110) — the `-C` flag (don't freeze the
  terminal while the status message shows) was unrecognized (`unknown flag
  -C`). Root cause: ztmux's `status_message_set` (`status.c:340`) had dropped
  the C's 5th int param `no_freeze`, so `display-message`'s `Cflag` had nowhere
  to go. Restored the parameter and split the body to match the C
  (`if (!no_freeze) tty.flags |= TTY_FREEZE;` then unconditional `TTY_NOCURSOR`);
  threaded `no_freeze` through the macro and all 12 call sites (11 pass `0` as
  the C does; `display-message` passes `Cflag`). Added `C` to the command's
  arg spec + usage (`aCc:d:lINpt:F:v`, `[-aCIlNpv] …`).

Round-5 fix:

- **missing global options** (1105–1108) — `show-options -g` was short four
  entries vs the vendored `options-table.c`. Ported the missing table entries
  faithfully (name/type/scope/default/text, in C order): `display-panes-format`
  (`options-table.c:826`), `focus-follows-mouse` (`:854`, FLAG default off),
  `initial-repeat-time` (`:873`, NUMBER 0..2000000 default 0), and refreshed the
  `update-environment` array default (`:1132`) which had dropped `MSYSTEM`,
  `WAYLAND_DISPLAY`, `XDG_CURRENT_DESKTOP`, `XDG_SESSION_DESKTOP`,
  `XDG_SESSION_TYPE`. (The theme-styled option defaults — `message-style`,
  `status-style`, `display-panes-*-colour`, `status-format[1..2]` — still differ
  because ztmux has no theme-colour subsystem yet; that is a separate gap. The
  `prompt-cursor-*` group needs the `OPTIONS_TABLE_IS_COLOUR` flag infra first.)

Round-4 fix:

- **buffer ordering / `paste_get_top`** (1100–1104) — `paste_cmp_times`
  (`paste.c:53`) sorted the `paste_by_time` RB tree *ascending* by `order`,
  but the C sorts *descending* (higher/newer `order` first). So `list-buffers`
  (no `-O`) listed oldest-first instead of newest-first, and `paste_get_top`
  (`RB_MIN` = "most recent automatic buffer") returned the *oldest* — a bare
  `paste-buffer`/`show-buffer` pasted the wrong buffer. Flipped the comparator
  to match C (`u32::cmp(&y, &x)`). Also fixed `list-buffers -r`: C's
  `sort_qsort` returns on `SORT_END` *before* honouring `reversed`, so bare
  `-r` (no `-O`) must not reverse — moved the reverse inside the `-O` arm.

Round-3 fixes:

- **`#{!!:…}` boolean-coerce operator** (1086–1089) — the `!!` modifier was
  never tokenized (missing from the double-char no-argument list), parsed, or
  applied, so it expanded to empty instead of `0`/`1`. Ported `FORMAT_NOT_NOT`
  (`vendor/tmux/format.c:5570`, `format_bool_op_1(es, copy, 0)`): added the
  flag, the `!!` arm in the double-char tokenizer, the modifier parse, and the
  apply branch mirroring the existing `#{!:…}` (`FORMAT_NOT`) path.

- **`#{c/f:…}` / `#{c/b:…}` colour→escape** (1090–1099) — the colour-to-SGR
  form was unimplemented (empty output). Root of a four-part gap, each fixed
  faithfully against the C:
  1. `colour_toescape` (`vendor/tmux/colour.c:295`) and its helper
     `colour_theme_terminal_colour` (`:101`) + `colour_theme_table` were never
     ported; added to `colour.rs` (with the `theme_colours` client field from
     `tmux.h:2293` and `COLOUR_FLAG_THEME`/`COLOUR_THEME_COUNT`).
  2. `format.rs` never parsed the `c` modifier's `f`/`b` argument
     (`FORMAT_COLOUR_ESC_FG/BG`) nor took the escape branch in the apply step.
  3. The single-char-with-args tokenizer set omitted `c`, so `#{c/f:…}`'s
     argument was never captured (added `c` → `"mCNSWPLst=peqc"`).
  4. Exposed two latent print-path bugs, both fixed to match the C:
     `cmdq_print_data` (`cmd-queue.c:837`) had drifted to take a `parse` param
     and was called with `0` (stravis-octal) instead of C's hard-coded `1`
     (raw → `utf8_sanitize`, so ESC renders as `_` like tmux); and
     `server_client_print` (`server-client.c:3014`) had dropped C's
     `if (size == 0)` guard, so an empty message underflowed `size - 1`
     (SEGV on any empty `display-message -p` output). Theme colour *names*
     (`#{c/f:themered}`) still need `colour_fromstring` theme support — a
     separate, larger gap; the `colour_toescape` theme branch is ported.

Round-2 fixes:

- **`#{s/…/…/}` unmatched/out-of-range backrefs** (841, 842) — the earlier
  regsub fix over-corrected: it skipped the digit for *every* backref. C only
  skips it when the group actually matched (`continue` inside the matched arm);
  an unmatched/out-of-range `\2` falls through and appends the literal digit
  (`\2` → `2`). Re-ported faithfully, incl. the `cp[1] != '\0'` guard.
- **`#{S:normal,active}` loop variant** (936, 937) — `format_loop_sessions`
  didn't split `fmt` into all/active via `format_choose` (the window/pane loops
  do). Added it.

### Layout divergences — resolved (0 remaining)

The former `select-layout` divergences with a **single** non-main pane
(secondary-pane sizing at 1001, 1004, 1023–1024, 1026, 1033–1035, 1039; and
`#{P:}` pane iteration order at 1025, 1027) have been reconciled against the
vendored tmux and now pass. All layout cases are byte-for-byte identical to
upstream.

The first expansion surfaced seven real port gaps, each pinned to a case and then
fixed:

- **`#{!:…}` logical-not operator** (548–550) — the `!` modifier wasn't tokenized
  (missing from the single-char list) nor applied. Ported `FORMAT_NOT`.
- **`#{s/…/…/}` regex backreferences** (566–568, 572) — `regsub_expand` continued
  without advancing past the digit and only for valid captures, so `\2\1` on
  `abcd` produced `b2a1d2c1`. Ported the C's `for (…; cp++)` semantics → `badc`.
- **`#{p-N:…}` left padding** (589, 592) — `utf8_rpadcstr` wrote `width` spaces
  instead of `width - n` (also overrunning its allocation). Now pads to the total
  field width.
- **`#{pane_at_top}` / `#{pane_at_bottom}`** (644–645) — emitted Rust
  `true`/`false` via `format!("{flag}")`; the C uses `%d` → `1`/`0`.
- **`#{S:}` / `#{W:}` / `#{P:}` loop variables** (676–678) — the loop modifiers
  now inject `loop_index` / `loop_last_flag` (vendor/tmux/format.c:4776).
- **Named buffers** (720–724) — root cause was `paste_get_name` assigning into a
  `MaybeUninit<paste_buffer>`'s `name` field, which dropped the uninitialized
  `Cow` and freed a garbage pointer (heap corruption → "empty buffer name" /
  crashes). Fixed with `ptr::write`. Also ported `list-buffers -O/-r`.
- **`main-vertical` / main-pane-width** (754) — the `cause` check in
  `layout_set_main_v` was inverted, overwriting a valid `main-pane-width` with the
  default 80. Fixed to match the C (default only on parse failure).

One case, `294_pane_cmd` (`#{pane_current_command}` on macOS), can flake by a
single case when the pane child hasn't finished `execvp` before the format is
read — a spawn/timing race, not a format divergence; it recovers on the next run.

### Earlier wins

The port is seeded from a transpile, so — unlike a from-scratch rewrite — a large
part of the format engine already works. The suite's job is now (a) to guard that
parity from regressing and (b) to keep growing coverage as more surface is exercised.

The suite has already paid off:

- `#{l:…}` (the literal operator) **crashed ztmux's server** — root-caused to a
  dropped pointer increment in `format_unescape`, fixed by a faithful re-port.
- **`405_select_layout`** — even-horizontal layout rounding was off by one column
  (`39|40` vs tmux's `40|39`). The port carried an *older* tmux algorithm that
  dumped the remainder on the last pane; ported the current C's leading-pane
  `remainder` distribution. Fixed. (This one was ALSO why comparing against a
  stale system tmux misled us — see the version note at the top.)

- **`294_pane_cmd.fmt`** — `#{pane_current_command}` reported the server binary
  (`ztmux`) instead of the pane's process (`sleep`) **on macOS only**. Traced to
  pane spawn: the forked child entered the child branch but never reached `execvp`
  (so the pane process stayed as ztmux). Root cause was `closefrom`: the macOS
  path looped `close()` up to the server-raised `RLIMIT_NOFILE` (millions of fds),
  so the child hung between fork and exec. Ported the `HAVE_LIBPROC_H` variant tmux
  actually compiles on macOS (`proc_pidinfo(PROC_PIDLISTFDS)` — close only open
  fds). The child now reaches `execvp`. Fixed.

Failing cases stay in the suite (never removed) — a green suite is earned by
porting the underlying code correctly, not by deleting the case.

## CI

The `Parity vs vendored tmux` job runs the full suite in CI and uploads the
failure log. Now that the suite is 100% green it acts as a blocking ratchet (like
strykelang's): any case that diverges from the vendored tmux fails the pipeline,
so a regression in ported behavior cannot land.

## Known gaps (proven-unported next-3.7 behaviour)

`parity/known_gaps/` is the inverse of `parity/cases/`: next-3.7 features ztmux
does **not** implement yet, each pinned by a case that is expected to *diverge*
from the reference. The 6 cases cover ~45 individual options/format-vars/commands
(pane scrollbars and the `scroll-to-mouse` command that drags their slider, the
theme system, copy-mode line numbers, floating-pane format vars, `switch-mode`,
…), each tied to an unported C area. Run them with the
inverted runner:

```sh
bash parity/run_known_gaps.sh   # "GAP" = still unported (expected); "CLOSED" = ported, promote it
```

The runner is an advisory tripwire — it exits non-zero only when a gap closes
(the feature got ported and its case should move to `parity/cases/`), so it never
reddens CI merely because the gaps still exist. See
[`parity/known_gaps/README.md`](known_gaps/README.md) for the full inventory and
proof. These gaps do not count against the 1173/1173 ported surface; they measure
the unbuilt surface beyond it.

## Growing the suite

Add a `.fmt` (one format) or `.sh` (one scenario) file under `parity/cases/`.
Keep them small and single-purpose; number-prefix by category. The sibling
suites scaled this to thousands of cases — the same shape scales here.

To record a newly-found divergence that ztmux does *not* yet match, add a `.sh`
case under `parity/known_gaps/` instead and confirm it with `run_known_gaps.sh`.
