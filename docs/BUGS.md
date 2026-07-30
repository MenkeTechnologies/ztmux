# Bug Fixes

Fixes to the ztmux port, most recent first.

## 2026-07-30

A parity round aimed at the areas the previous bug rounds came out of, rather than
at new commands: the copy-mode command table and its format variables, the grid as
read back through `capture-pane`, the signed offset arithmetic in layout and
resize, and the popup/menu argument parsers. Coverage before the round was 3 cases
against 91 copy-mode table entries and 2 cases that called `capture-pane` at all.
The 32 new cases (`parity/cases/1439`–`1470`) found five bugs, two of them server
crashes; the suite is 1166/1166.

### 1. `append-selection` took the server down

- **Symptom:** in copy mode, `copy-line` followed by `begin-selection` and
  `append-selection` exited the server. Any append onto an existing buffer did it;
  an append with no buffer yet did not.
- **Root cause:** `paste_set` (`src/ported/paste.rs`) ended with
  `notify_paste_buffer(name, …)` using the **caller's** name. C uses `pb->name` —
  the copy `paste_set` itself just made (`vendor/tmux/paste.c`). That difference is
  invisible in C, where `paste_get_top` hands back an `xstrdup`, but
  `window_copy_append_selection` here borrows the name straight out of the buffer
  it is about to replace, and `paste_free(old)` drops that string before the notify
  reads it: a use-after-free.
- **Fix:** notify with the new buffer's own name, as the C does.
- **Pinned by:** `parity/cases/1448_copy_mode_copy_to_buffer.sh`.

### 2. `capture-pane -S -5` took the server down

- **Symptom:** any negative `-S`/`-E` (capture N lines back into history) exited
  the server in a debug build.
- **Root cause:** `top = gd->hsize + n` with `n` an `int` and `hsize` a `u_int` is
  an unsigned wrap in C that lands on the intended history line. Written as a plain
  Rust add (`src/ported/cmd_capture_pane.rs`) it is an overflow panic. Same shape as
  the earlier `tty_cursor` and `previous-prompt` fixes.
- **Fix:** `wrapping_add_signed` on both the `-S` and `-E` paths, with the reason in
  a comment.
- **Pinned by:** `parity/cases/1454_capture_pane_ranges.sh`.

### 3. `#{selection_mode}` and `#{search_timed_out}` never expanded

- **Symptom:** both format variables expanded to nothing where tmux prints
  `char`/`word`/`line` and `0`/`1`.
- **Root cause:** `window_copy_formats` (`src/ported/window_copy.rs`) had dropped
  the `selflag` switch and the timeout entry present at `window-copy.c:1139`
  and `:1152`.
- **Fix:** ported both blocks.
- **Pinned by:** `parity/cases/1443_copy_mode_selection_formats.sh`,
  `1447_copy_mode_search_formats.sh`.

### 4. A failed search kept the previous search's match count

- **Symptom:** after a search that matched nothing, `#{search_count}` still
  reported the count from the previous search instead of expanding empty.
- **Root cause:** `window_copy_clear_marks` (`window-copy.c:4805`) resets
  `searchcount = -1` and `searchmore = 0` before freeing the mark array; the port
  only freed the array, and `-1` is what suppresses the format entirely.
- **Fix:** reset both fields, as the C does.
- **Pinned by:** `parity/cases/1447_copy_mode_search_formats.sh`.

### 5. Percentages over 100% were rejected

- **Symptom:** `resize-pane -x 150%` failed with `width too large`; tmux resolves
  it to 120 and lets the layout clamp it to the window.
- **Root cause:** `args_string_percentage` and `args_string_percentage_and_expand`
  (`src/ported/arguments.rs`) bounded the percentage numerator at 100. The C bounds
  it at 1000 (`arguments.c:1013`, `:1081`) and clamps the *product* against
  min/max. A unit test had encoded the wrong bound.
- **Fix:** ported the 1000 bound to both functions; the test now asserts the C's
  behaviour (150% of 200 is 300; only a numerator past 1000 is "too large").
- **Pinned by:** `parity/cases/1462_resize_pane_bounds.sh`.

Two divergences the round proved are unported features rather than defects went to
`parity/known_gaps/`: 11 `send-keys -X` commands absent from the copy-mode table
(`recentre-top-bottom`, `refresh-{on,off,toggle}`, `scroll-exit-{on,off,toggle}`,
`cursor-centre-{vertical,horizontal}`, `scroll-to-mouse`, `selection-mode`) and
`capture-pane -F/-H/-L/-M`, the last of which also needs `GRID_LINE_HYPERLINK` and
the mode `get_screen` callback. Both were ported immediately afterwards — see the
next section — so the only one still open is `scroll-to-mouse`, which belongs to
the unported pane-scrollbars subsystem.

### 6. `recentre-top-bottom` took the server down (found while porting it)

- **Symptom:** the first `recentre-top-bottom` from any scrolled-back view exited
  the server.
- **Root cause:** each branch adjusts the cursor row by the **signed** change in
  the scroll offset, which the C writes as `data->cy = cy + (data->oy - oy)` in
  `u_int`. Scrolling up lowers `data->oy`, so that inner subtraction is a negative
  delta carried as a huge unsigned value and the outer add wraps it back down
  (23 + (5 - 17) = 11). Both operations panic in a Rust debug build, and the
  common case — recentring a view that is scrolled back — hits them immediately.
- **Fix:** `wrapping_add`/`wrapping_sub` on all three branches
  (`src/ported/window_copy.rs`), with the reason in a comment.
- **Pinned by:** `parity/cases/1471_copy_mode_recentre_and_centre.sh`, which drives
  two full cycles plus the clamped cases at the top and bottom of the history.

## 2026-07-30 (port round: the two gaps above)

Closing the two gaps the parity round recorded. Nothing here was a defect in
already-ported code except bug 6 above; the rest is next-3.7 surface that had no
ztmux counterpart.

- **Copy-mode command table** — ported the 10 missing entries: `refresh-on`,
  `refresh-off`, `refresh-toggle`, `scroll-exit-on`, `scroll-exit-off`,
  `scroll-exit-toggle`, `recentre-top-bottom`, `cursor-centre-vertical`,
  `cursor-centre-horizontal` and `selection-mode`. `refresh-from-pane`, which the
  port carried and next-3.7 does not, is gone: upstream replaced that one-shot
  reclone with the automatic refresh below, and the `r` binding in both copy-mode
  key tables now maps to `refresh-toggle` as the C does (`key-bindings.c:538`,
  `:648`), with `C-l`/`M-l` added (`:515`, `:516`).
- **The automatic refresh subsystem behind those commands** —
  `window_copy_sync_snapshot`, `window_copy_sync_backing`, `window_copy_do_refresh`,
  `window_copy_refresh_arm`, `window_copy_refresh_timer`, `window_copy_refresh_start`
  and `window_copy_refresh_stop`, plus the `refresh_timer`/`refresh_active` and
  `recentre_state`/`recentre_line` fields. The incremental sync needs the grid's
  monotonic scroll counters (`scroll_added`, `scroll_collected`,
  `scroll_generation`, `tmux.h:898`), which are now maintained at all five C sites
  (`grid.c:470`, `:508`, `:521`, `:559`, `:1611`).
- **`grid_collect_history` gained its `all` parameter** (`grid.c:447`) and the
  caller that passes it: `session_update_history` (`session.c:765`), which applies
  a changed `history-limit` to every pane in a session, wired to the option hook at
  `options.c:1313`. Neither existed here before, so raising `history-limit` never
  collected the history that no longer fit.
- **`capture-pane -F/-H/-L/-M`** — the flag string was `ab:CeE:JNpPqS:Tt:` against
  the C's `ab:CeE:FHJLMNpPqS:Tt:`. Ported `cmd_capture_pane_hyperlinks`
  (`cmd-capture-pane.c:111`), the per-line number and flag prefixes, and the `-M`
  screen selection, which needed two pieces of substrate: the `GRID_LINE_HYPERLINK`
  line flag (`tmux.h:804`, set at `grid.c:189`) and the `get_screen` callback on
  `struct window_mode` (`tmux.h:1180`), implemented by copy mode and view mode.
- **Still unported:** `scroll-to-mouse`. It drags the scrollbar slider, so it needs
  `wp->sb_slider_h` (`tmux.h:1301`) and `tty.mouse_slider_mpos` (`tmux.h:1769`) —
  state that only exists with the pane-scrollbars subsystem, recorded separately in
  `parity/known_gaps/opt_pane_scrollbars.sh`.

Covered by `parity/cases/1471`–`1477`: the recentre cycle and both centre commands,
the scroll-exit flag through all three commands and `copy-mode -e`, `selection-mode`
with every spelling of its argument, the refresh commands, and `capture-pane`
`-L`/`-F`/`-H`/`-M` including their combinations.

## 2026-07-13

A memory-ownership round: convert C `char *` struct fields to owned Rust types and
delete the hand-rolled `free()` calls. Doing so surfaced a family of faults that all
share one shape — **a C idiom that is silently unsafe once the struct holds a Rust
type**. Two new build gates were added so each class fails the build if it returns.

The crash surface was found by driving the whole command set against a private
socket and, for the client-only paths (modes, redraw, status), against a real client
on a pty. Aborts were keyed on crash reports rather than on "is the server still up",
since a dying client can legitimately take the server with it.

### 1. `new-session -t <new-group>` killed the server

- **Symptom:** `ztmux new-session -t ggg`, where no session `ggg` exists, exited the
  server (`server exited unexpectedly`). tmux instead creates session `ggg-0` in a new
  session group named `ggg`.
- **Root cause:** two independent faults on the same line of execution.
  1. `session_group_find` (`src/ported/session.rs`) mirrored C's throwaway stack struct
     used as the `RB_FIND` key: `struct session_group sg; sg.name = name;`. In Rust
     `(*sg).name = …` is a *place assignment*, so it **drops the previous value** — and
     the previous value was uninitialized stack garbage. The garbage happened to look
     like a `Cow::Owned`, so it called `free()` on a pointer Rust never allocated
     (`POINTER_BEING_FREED_WAS_NOT_ALLOCATED`, SIGABRT).
  2. `session_group_synchronize_to` then hit the `TAILQ_FOREACH` semantic below.
- **Fix:** search by key with `rb_find_by` — the same O(log n) descent with no
  fabricated key node and no `transmute`. Pinned by `session_group_find` unit test
  (mutation-checked: reversing the comparator fails it).

### 2. `TAILQ_FOREACH`/`RB_FOREACH` "not found" returned an arbitrary element

C's `TAILQ_FOREACH` leaves the loop variable **NULL** when the loop runs to completion,
and every caller branches on that NULL. A Rust `for` loop that assigns each element
instead retains the **last one visited**, so "not found" silently became "some arbitrary
element". Five ports had it:

- **`cmd_find_client`** — `lock-client -t nosuch` returned a **session-less** client, and
  `server_lock_client` dereferenced `c->session`: **server dead**. Worse than the crash,
  every `CMD_CLIENT_TFLAG` command shared it, so `detach-client -t <typo>` silently acted
  on the *wrong client* instead of erroring.
- **`session_group_synchronize_to`** — a group whose only member is `s` selected `s`
  itself, so the session was synchronized *from itself*, wiping its own window list;
  `RB_MIN(&s->windows)` then returned NULL. This is the second half of bug 1.
- **`window_pane_set_mode`** — a pane already in some other mode **reused that entry**
  instead of creating a new one, binding the new mode to the previous mode's `data`: a
  type confusion.
- **`cmd_find_inside_pane`** — returned an unrelated pane, so the `TMUX_PANE` fallback
  never ran.
- **`format.c` window-stack index** — reported the full stack length instead of `0` when
  the winlink is not in the stack.

- **Fix:** `.find(…)` / `.any(…)`, which reproduce C's "first match, else none".

### 3. Destroying a pane in a mode rebuilt the mode against the dead window

- **Symptom:** a null dereference under
  `window_destroy → window_pane_destroy → … → window_customize_build`.
- **Root cause:** `window_pane_destroy` called `window_pane_reset_mode_all`. C calls
  **`window_pane_free_modes`**, which the port was missing entirely. `reset_mode_all` is
  the *interactive* path: for each mode popped it resizes the next mode, redraws and
  notifies — so tearing down a pane rebuilt the customize-mode tree against a window that
  was already gone. C's only `reset_mode_all` callers are spawn / capture-pane /
  copy-mode, which the port already matched.
- **Fix:** port `window_pane_free_modes` (frees each entry, resets `wp->screen`, no
  resize/redraw/notify) and call it from `window_pane_destroy`.

### 4. C-allocated structs that hold a Rust type (new gate)

- **Symptom:** `choose-client` with a client attached killed the server.
- **Root cause:** `Vec`, `String`, `CString` and `Box` all require a **non-null** data
  pointer. `xcalloc` (libc `calloc`) returns all-zero bytes, so such a field comes out
  with a NULL pointer — a value the type system says cannot exist. Nothing complains at
  the allocation; it detonates later, far from the cause. `window_client_modedata` holds
  `item_list: Vec`, so the first `item_list.drain(..)` in `window_client_build`
  dereferenced null. `window_buffer_itemdata` (`name: String`) and `sixel_image`
  (`colours: Vec`) had the same defect, papered over by assigning a fresh empty value
  before use.
- **Fix:** build each through `Box::new(…)` with every field a valid Rust value and
  reclaim with `Box::from_raw`, so `Drop` frees them.
- **Why it matters for the rest of the migration:** the moment a `char *` field becomes an
  owned `CString`, every existing C-style allocation of its struct silently becomes UB.
  That is exactly how `window_client_modedata` broke. `tests/no_c_alloc_for_rust_types.rs`
  now **fails the build** when a struct holding a `Vec`/`String`/`CString`/`Box` is
  allocated via `xcalloc` / `zeroed` / `MaybeUninit`.

### 5. Truncated `key_code` let a mouse event kill a window (new gate)

- **Symptom:** none observed in normal keyboard use — found by reading the dispatch.
- **Root cause:** tmux dispatches keys with `switch (key)` over the full 64-bit
  `key_code`. Five ported handlers (`window_tree`, `window_customize`, `window_client`,
  `window_buffer`, `popup`) matched **`key as u8`** against byte literals, discarding the
  top bits. `KEYC_*` codes run sequentially from `KEYC_BASE` (0x10e000), so **18 real keys
  alias an ASCII command letter**:

  | key | truncates to | command it runs |
  | --- | --- | --- |
  | `KEYC_MOUSEUP11_STATUS_DEFAULT` (0x10e078) | `'x'` | **Kill** prompt |
  | `KEYC_TRIPLECLICK7_STATUS_LEFT` (0x10e178) | `'x'` | **Kill** prompt |
  | `KEYC_DOUBLECLICK11_PANE` (0x10e158) | `'X'` | **Kill Tagged** |
  | `KEYC_MOUSEMOVE_BORDER` (0x10e00d) | `CR` | run the command on the row |

  i.e. a mouse event reaching those handlers could kill a window or a session.
- **Fix:** gate each dispatch on `key < 0x80`, so only a genuine bare ASCII byte reaches
  the byte-literal arms — what C's full-width `switch` does. (`mode_tree.rs` already had
  the other correct shape: compare against `u64` constants.)
  `tests/no_key_code_truncation.rs` now fails the build on any `match key as u<N>`.

### Memory ownership converted this round

`wait_channel` (+ `wait_item`), `window_client_modedata`, `window_buffer_modedata` (+
`window_buffer_itemdata`), `window_tree_modedata`, `window_customize_modedata`,
`sixel_image`. Owned `char *` fields became `CString`; the structs are built with
`Box::into_raw` and reclaimed with `Box::from_raw`, so `Drop` frees them instead of the
hand-rolled `free()` calls. `cmd_wait_for.rs` no longer contains a single `free_`,
`xcalloc` or `xstrdup`.

Note: `wait_channel` was `Box::leak`'d but freed with libc `free_`. That only worked
because the global allocator (`MyAlloc`, `src/main.rs`) is hardwired to libc
`malloc`/`free` — and it skipped `Drop` entirely, so it would have leaked the `CString`
the instant the field was converted.

### Known open

A non-deterministic server exit remains, reachable by driving tree-mode with keys. It is
**not** a memory fault — no abort, no crash report, no panic; the server exits cleanly —
and it is **pre-existing** (`HEAD` before this round fails identically). It has not been
isolated and is not claimed fixed.

## 2026-07-02

This round paired three harnesses — a parity-case expansion (689 → 1080 cases),
an in-process fuzz harness over the pure parsers/format engine, and a fan-out of
adversarial Rust-vs-C audits over the largest modules. Between them they
root-caused ~40 divergences; the headline fixes are below, the rest summarised at
the end.

### 1. `split-window -f` crashed the whole server

- **Symptom:** `split-window -f` (full-size split) when the window already had a
  pane exited the server (`server exited unexpectedly`), taking every session
  with it. A single-pane `-f` split worked; the crash needed a pre-existing split.
- **Root cause:** `layout_resize_child_cells` (`src/ported/layout.rs`) computes
  `available -= (lcchild->sx + 1)` over the children. C's `available` is `u_int`,
  so a transient over-subscription during the `-f` restructure wraps harmlessly
  and is corrected by the follow-up resize. The Rust port used a checked `-=`,
  which panicked (`attempt to subtract with overflow`).
- **Fix:** match C's `u_int` wrap with `wrapping_sub` on both the `sx` and `sy`
  subtractions. ztmux now produces a byte-identical `window_layout` to tmux for
  the multi-pane full-size split. Pinned by parity case `1319_split_full.sh`.

### 2. Invalid UTF-8 in a format string crashed format expansion

- **Symptom:** any format containing a non-UTF-8 byte in a `#{…}` variable /
  modifier position aborted the process. Formats are re-expanded constantly (the
  status bar every redraw, pane titles, `display-message`, hooks), so this was a
  server-crash surface. Found by the fuzz harness.
- **Root cause:** `cstr_to_str_` — the *`Option`-returning, ostensibly fallible*
  C-string converter (`src/lib.rs`) — called `.expect("bad cstr_to_str")`, so it
  panicked on invalid UTF-8 instead of returning `None`. `format_find` and the
  `c/f:` / `=N:` modifiers route arbitrary bytes through it.
- **Fix:** `cstr_to_str_` returns `None` on invalid UTF-8 (a fallible conversion
  must not panic on its one failure mode). `format_find` skips option lookups for
  a non-UTF-8 key (matching C's raw-`char*` compare, which finds no match); the
  colour and width/trim modifiers fall back to `""` / byte-length. C operates on
  raw bytes throughout, so ztmux now degrades gracefully instead of crashing.

### 3. `screen_write_clearstartofscreen` inverted a null check

- **Symptom (latent):** on erase-to-start-of-screen (`ESC[1J`) with a sixel image,
  the pane was never marked for redraw (stale image persisted), and the guard's
  one admitted case dereferenced a null `wp`.
- **Root cause:** `src/ported/screen_write.rs:1807` used `(*ctx).wp.is_null()`
  where the C (`screen-write.c:1992`) and all 21 sibling blocks use
  `ctx->wp != NULL`.
- **Fix:** restored `!(*ctx).wp.is_null()`.

### 4. `set-flags` was a total no-op

- **Symptom:** `refresh-client -f …`, `attach -f read-only`, and control-mode
  flags (`no-output`, `pause-after`, …) never took effect.
- **Root cause:** `server_client_set_flags` (`src/ported/server_client.rs:3343`)
  inverted the `strsep` loop condition (`next.is_null()` instead of
  `!next.is_null()`), so the flag-parsing body never ran.
- **Fix:** `!next.is_null()`, matching C `server-client.c:2861` and every other
  `strsep` loop in the port.

### 5. `tty_emulate_repeat` off-by-one + `u32` underflow

- **Root cause:** `while { n -= 1; n > 0 }` ran the body `n-1` times and
  underflowed on `n == 0`; C `tty.c:914` is `while (n-- > 0)` (runs it `n` times).
- **Fix:** `for _ in 0..n { … }` (`src/ported/tty.rs`). Affects insert/delete
  char/line on terminals lacking the parameterised capability.

### 6. `new-window` usage string dropped `[argument ...]`

- **Fix:** restored `[shell-command [argument ...]]` to match C
  (`cmd-new-window.c:44`). Pinned by parity case `1389`.

### Batch — adversarial Rust-vs-C audit (version-independent divergences)

A fan-out of read-only audits over the biggest modules (each diffing the Rust
against the exact `vendor/tmux` C function) surfaced a cluster of transcription
bugs, all fixed faithfully and, where unit-testable, pinned by regression tests:

- **window_copy:** `write_lines` rendered the same line N times (ignored loop
  var); `select-line` off-by-one selected an extra line; `move_after_search_mark`
  compared pointers instead of the byte values; `cursor_up`/`down` dropped the vi
  `scroll_only` pre-move.
- **format:** trailing-`#` read past the NUL (missing end-of-string guard); the
  `p` modifier `break` exited the whole modifier loop.
- **format_draw:** `format_width` spun forever on a truncated trailing multibyte
  (a rewind with no advance); `STYLE_LIST_FOCUS`/`LEFT_MARKER` `break`s aborted
  the parse loop.
- **input_keys:** extended CSI-u key missing its terminating `u`; `vt10x` dropped
  the `\n` C0 case; `mode1` wrongly required Meta clear; `backspace` decode used a
  stale clamp; paste keys missing the `KEYC_IMPLIED_META` entries.
- **resize/spawn:** inverted per-client size-clamp guard (null-deref); respawn
  wrongly rejected an all-dead window; empty window name not defaulted;
  `spawn_pane` dropped the `item == NULL` branch.
- **grid/utf8:** regional-indicator width forced to 2 (should be 1); `grid_reflow`
  join and `grid_string_cells` dereferenced before the null/range guard.
- **mode_tree/window_customize/window_tree/window_buffer:** empty-tree keypress
  panic; up/down underflow on an empty list; a customize-mode filter that failed
  spun forever (missing iterator advance); tag `-1` `<<` overflow; inverted
  activity sort (ascending instead of most-recent-first); `break`s that skipped
  the redraw; an `edit_close` null-deref.
- **compat / pure fns (the "shore up the floor" pass):** `b64_pton` rejected
  digits and had no `=` padding, `b64_ntop` return off-by-one; `strnvis` ignored
  its length bound; `strtonum` overflow reported `invalid` instead of
  `too large`/`too small`; `attributes_fromstring` rejected consecutive
  delimiters; `colour_byname`/`colour_fromstring` were case-sensitive on
  grey/gray and panicked on multibyte input; `ibuf_dynamic` capped `max` at `len`;
  `regsub`/`names`/`grid_reader` faithful re-ports.

Test coverage grew alongside: unit tests 520 → **1253**, parity cases 689 →
**1080** (100%), plus an opt-in fuzz harness (`src/fuzz_smoke.rs`) and a
single-case parity verifier (`parity/verify_one.sh`).

## 2026-07-01

### 1. SGR mouse truncation froze TUI panes

- **Symptom:** rich crossterm/ratatui TUIs (storageshower, iftop-rs) froze the
  moment the pane got a mouse event, a click, or a focus change. Keyboard input
  still worked. `refresh-client`, detach/reattach, and SIGWINCH all failed to
  recover it, and only the affected pane froze.
- **Root cause:** `xsnprintf__` was corrected to return the formatted length
  *excluding* the terminating NUL (like C `snprintf`), but the SGR-mouse encoder
  still carried a stale `- 1`, which dropped the sequence's final byte — the
  `M`/`m` terminator. ztmux wrote `\033[<35;69;44` (no terminator) to the pane;
  crossterm recognised the `\033[<` SGR-mouse prefix and blocked in `read()`
  waiting for an end byte that never came.
- **Fix:** removed the `- 1` in `input_key_get_mouse` — `src/ported/input_keys.rs`.
  Regression test: `test_get_mouse_sgr_keeps_terminator`.

### 2. ztmux hijacked real tmux's socket

- **Symptom:** `ztmux ls` / `list-keys` / creating multiple sessions returned
  "server exited unexpectedly"; ztmux and tmux could not run side by side.
- **Root cause:** ztmux resolved its default socket from `$TMUX`, so when launched
  inside a tmux pane it connected to tmux's server and spoke protocol 8 at it.
- **Fix:** ztmux resolves its socket only from `$ZTMUX` (never `$TMUX`), and
  advertises both `$TMUX` (ecosystem compatibility) and `$ZTMUX` (its own handle)
  to panes — `src/ported/tmux.rs`, `src/ported/environ.rs`,
  `src/ported/server_client.rs`.

### 3. Version string broke config version-gates

- **Symptom:** version-gated user config (`tmux -V | awk '{print ($2>=3.1)}'`)
  sourced the wrong files under ztmux — legacy `tmux_lt_*` confs instead of the
  modern `tmux_ge_*` ones.
- **Root cause:** `ztmux -V` reported the crate version `0.1.0`, so the awk gate
  evaluated to 0.
- **Fix:** report `3.7.0` (matches the installed tmux) across `Cargo.toml`,
  `package.json`, `Cargo.lock`, and the man pages. Also fixes the bogus
  `tmux 0.1.0` string in the XTVERSION / `TERM_PROGRAM_VERSION` reply that apps
  read.

### 4. Red-black tree delete rebalancing rotated around the wrong node

- **Symptom:** crashes (invalid node dereference / segfault) on certain
  delete-then-reinsert sequences — e.g. rebinding a key that already exists
  (`bind-key l ...`), which removes the old node and inserts a fresh one.
- **Root cause:** in `rb_remove_color`, the right-hand (mirror) rebalancing
  branch called `rb_rotate_left(head, oright)` where it should rotate around the
  sibling `tmp`. The wrong pivot corrupted parent/child links; the tree stayed
  usable for some shapes but broke for others, eventually dereferencing a bogus
  node.
- **Fix:** `rb_rotate_left(head, oright)` → `rb_rotate_left(head, tmp)` —
  `src/ported/compat/tree.rs`. Added RB-invariant tests (`black_height` checker,
  `remove_then_insert_hl_keeps_tree_valid`, plus an LCG-shuffled delete stress
  test). Commit `ef408be6f9`.

### 5. `log_debug!` took a mutex on every call

- **Root cause:** the logging-disabled fast path locked the `LOG_FILE` mutex on
  every call; on the hot parse/redraw path that is a mutex lock/unlock thousands
  of times per frame.
- **Fix:** gate on the atomic `LOG_LEVEL` first (matches C tmux's
  `if (log_level == 0) return;`) before touching the mutex — `src/ported/log.rs`.

### 6. `client-panic.txt` written into the cwd

- **Symptom:** panic dumps landed wherever ztmux was launched (home, Desktop).
- **Fix:** write to `std::env::temp_dir()` (honours `$TMPDIR`, falls back to
  `/tmp`) — `src/ported/tmux.rs`.

### 7. `#{l:…}` format literal crashed the server

- **Symptom:** a `#{l:…}` format expansion crashed the server.
- **Root cause:** a dropped pointer increment (`s = s.add(1)`) in
  `format_unescape` left the scan pointer unadvanced, running off the buffer.
- **Fix:** restore the increment — `src/ported/format.rs`. Commit `7a3fd1f983`.
  Found via the parity suite.

### 8. even-horizontal / even-vertical layout rounding

- **Symptom:** `select-layout even-horizontal` / `even-vertical` sized cells
  wrong (off-by-one rounding), diverging from tmux.
- **Root cause:** incorrect `each`/remainder split in `layout_spread_cell`.
- **Fix:** faithful C port of the size/remainder distribution —
  `src/ported/layout.rs`. Commit `b5099243e9`. Found via the parity suite.

### 9. Pane spawn hung on macOS (`closefrom`)

- **Symptom:** spawning a pane could hang.
- **Root cause:** `closefrom` looped `0..getdtablesize()` calling `close()` on
  every possible fd, which is pathological when the fd limit is very large.
- **Fix:** faithful macOS libproc port — enumerate the actually-open fds via
  `proc_pidinfo`/`PROC_PIDLISTFDS` and close only those (with a fallback) —
  `src/ported/compat/closefrom.rs`. Commit `3ec5359692`.
