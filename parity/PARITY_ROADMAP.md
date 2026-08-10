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

**1194/1194 cases pass (100%) vs the vendored tmux — zero known divergences.** The
suite grew from 122 → 380 → 646 → 661 → 665 → 675 → 680 → 684 → 686 → 689 → 774 → 840 → 900 → 1080 → 1107 → 1115 → 1121 → 1123 → 1130 → 1134 → 1166 → 1173 → 1178 → 1180 → 1183 → 1188 → 1193 → 1194 cases.

Case **1498** is structural rather than another probe: it diffs the *whole*
default binding table against next-3.7's, which nothing had done before. The
anti-drift gate (`tests/ported_fn_names_match_c.rs`) compares function *names*,
and a default binding is *data*, so the ~283 binding strings had never been
compared to `key-bindings.c` — five had drifted (a wrong command on
`MouseDown1Status`, a missing `#{alternate_on}` on `WheelUpPane`, a spurious `-O`
on five menus, a hand-written session menu, and 16 bindings missing the `--`
before their argument). It compares what `list-keys` prints rather than the
source text, so it is a diff of what each binary actually *parsed*: cosmetic
transcription differences that parse to the same command list are canonicalised
away by the round trip. 243 of the 283 bindings are compared; the 40 excluded are
listed by key in the case with the reason for each, and the list shrinks as the
features behind them land. The output is sorted, because `list-keys` walks each
table in key-code order and ztmux's flat `keyc` enum orders differently from the
C's type-shifted one — so the case compares the set of bindings and their
commands, not their order, until that encoding migrates. Both halves were
mutation-tested: reintroducing the `MouseDown1Status` command and dropping one
`--` each turn the case red.
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

`scroll-to-mouse` is the one command that did not graduate. Pane scrollbars are now
ported, so `wp->sb_slider_h` exists, but dragging the slider also needs
`tty.mouse_scrolling_flag` / `tty.mouse_slider_mpos` and the
`KEYC_MOUSE_LOCATION_SCROLLBAR_*` key codes, which ztmux's six-location `keyc`
mouse table has no way to name. It is recorded in `docs/BUGS.md`.

Round-10 port — pane scrollbars:

The `pane-scrollbars*` gap closed: the four options, the scrollbar scene
(`redraw_mark_pane_scrollbar`, `redraw_draw_scrollbar_span`,
`redraw_pane_scrollbar` and the `REDRAW_SPAN_SCROLLBAR` span type), the
`window_pane_scrollbar_*` predicates and the auto-hide timer, the column a
reserved bar takes out of the pane (`layout_fix_panes`, the split/resize
minimums, `window_pane_full_size_offset`, `window_visible_ranges`), and the
mouse hover that reveals an auto-hiding bar.

Case 1483 covers the reserved column, which is visible in `pane_width` /
`pane_left`. The bar itself is drawn straight to the client's terminal and never
enters the pane's grid, so `capture-pane` on the pane cannot see it. Case 1484
observes it the way `regress/am-terminal.sh` observes drawn output — a second
server whose only client is attached inside a pane of the first, then
`capture-pane -e` on that outer pane — which makes the trough, the slider and
the padding column byte-comparable against the reference for every position,
width, padding and scrollbar mode.

Round-11 port — tree-mode preview and styles:

The `opt_tree_mode` gap closed. Five next-3.7 window options landed with the
drawing behind them: `tree-mode-selection-style` and `tree-mode-border-style` in
`mode_tree_draw` (`mode-tree.c:840`, `:842`, `:989`, `:1002`–`:1016`), and
`tree-mode-border-style` / `tree-mode-preview-style` / `tree-mode-preview-format`
in `window_tree_draw_session` and `window_tree_draw_window` (`window-tree.c:618`,
`:635`, `:766`, `:783`) through a new `window_tree_border_cell`
(`window-tree.c:508`) and the next-3.7 `window_tree_draw_label`
(`window-tree.c:478`), which frames the label with the border cell, clears the
label row to the border background, and draws the expanded format with
`format_draw` instead of the previous hand-built `idx:name` string.
`screen_write_vline` regained the `const struct grid_cell *` parameter its own
doc comment already claimed (`screen-write.c:795`), so the preview separators and
the `<`/`>` arrows take the border style.

That work surfaced a real bug in the port. `options_string_to_style`
(`options.c:1010`) sets `o->cached = (strstr(s, "#{") == NULL)` — a style with no
format in it parses once and is cached, one that must be expanded never is. The
port had the test inverted, so every style whose value contains `#{` was cached
after being parsed *literally* and was never expanded against the format tree.
That is exactly the shape of these two defaults — `tree-mode-selection-style` is
`#{E:mode-style}` and `tree-mode-preview-style` picks its colour from
`#{?…pane_active…}` — and of any user style written as a format. Fixed at
`options.rs:1239`.

`switch-mode-match-style` was in the table with its C default and nothing reading
it; Round-12 ported `window-switch.c`, whose `window_switch_draw_screen`
(`window-switch.c:289`) is that reader, so the option is live.

Case 1485 covers the option surface (defaults, the `,` separator under `-a`,
style validation, `#{E:}` resolution). Cases 1486 and 1487 cover the preview as
drawn, through the same nested-client trick as 1484: 1486 the session preview
under every border/preview-style/format variation, 1487 the selection style and
the window preview, whose format and styles come from each *pane's* options. The
item list above the box still differs (next-3.7 builds it from a prefix format
this port does not have), so both cases capture the preview box only, plus the
leading SGR run of the selected line for the selection style.

Not ported, and still absent: `mode_tree_draw_help` and the `?` overlay,
`window_tree_draw_info` and the `preview_is_info` toggle, `window_tree_help`,
and `window_tree_sort`/`window_tree_swap` (next-3.7 replaced mode-tree's static
sort list with `sortcb`/`swapcb`/`helpcb`). The
`#[#{E:tree-mode-border-style},acs]x` info/help format strings in `window-tree.c`,
`window-client.c`, `window-buffer.c` and `window-customize.c` live inside exactly
those unported functions, so none of them are in the tree yet. The
`mode_tree_prompt_*` functions were in that list too until Round-12 built the
`struct prompt` object they need.

Round-12 port — `prompt.c` as an object, then `switch-mode`:

The last `parity/known_gaps/` case was `switch-mode`, and it could not be closed
on its own. `window-switch.c` drives the prompt as an **object** — `prompt_create`,
`prompt_update`, `prompt_incremental_start`, `prompt_draw`, `prompt_key`,
`prompt_mouse`, `prompt_free` — while this port still carried the pre-split
design: nineteen `prompt_*` fields on `struct client` and twenty-four
`status_prompt_*` functions in `status.rs` taking a `*mut client`. So the round is
two ports.

`prompt.c` became `src/ported/prompt.rs`. `struct prompt` owns the string, the
buffer and cursor index, a `cmd_find_state`, the callbacks, the styles and cursor
styles/colours, the key mode, the word separators, the per-type history index,
the `C-w` copy buffer and the completion list; `struct client` keeps one
`prompt: *mut prompt` and `struct status_line` gains `prompt_cx` (`tmux.h:2014`).
All 13 `PROMPT_*` flags are present (this port had 5) — `PROMPT_COMMANDMODE`
replaces the separate `enum prompt_mode`, and `PROMPT_QUOTENEXT` (`C-v`),
`PROMPT_BSPACE_EXIT`, `PROMPT_NOFREEZE`, `PROMPT_ACCEPT`, `PROMPT_ISPANE`,
`PROMPT_ISMODE` and `PROMPT_EDITARROWS` are new behaviour. `prompt_draw` takes a
`prompt_draw_data` (a write context, a row, an x range and a cursor-column
out-parameter), which is what lets the status line, a mode tree and switch mode
run the same editor; it expands `message-format` rather than the raw prompt
string, so `#{message}`, `#{prompt_input}`, `#{prompt_flags}`, `#{prompt_type}`
and `#{command_prompt}` all reach a prompt now. Completion is upstream's —
commands only, at offset zero, drawn as an inline underlined list — replacing the
session/window menu this port had. `prompt-history.c` came with it as
`src/ported/prompt_history.rs`, including the three accessors
(`prompt_history_size`/`_get`/`_clear`) `clear-prompt-history` now goes through.
`status.c` keeps the thin wrappers upstream keeps, plus `status_message_area`
(`status.c:413`). `mode_tree_set_prompt` and friends (`mode-tree.c:1068`–`1172`)
landed too, so a mode tree owns its prompt and draws it on its own row, and
`window-tree`/`window-customize` moved off `status_prompt_set` onto it.

`window-switch.c` then became `src/ported/window_switch.rs`, with
`cmd_switch_mode_entry` (`cmd-choose-tree.c:87`) registered in `cmd.rs` and the
`Tab`/`BTab` prefix bindings from `key-bindings.c:405`. Case 1488 covers the
command surface (flag set, usage, entering and leaving the mode, the bindings,
the option); case 1489 covers the picker as drawn — list, selection, incremental
prompt row and `switch-mode-match-style` on the fuzzy-matched columns; case 1490
covers `-k`, which the bindings rely on to dispose of the scratch pane and which
needed `window_mode_entry.kill` (`window.c:1380`) and the `server_kill_pane` at
the end of `window_pane_reset_mode` (`window.c:1428`). 1489 and 1490 are captured
through a nested client the way 1484/1486 do.

The round also closed a hole in the suite itself. Nothing drove `prompt_key`,
because `send-keys` writes into a pane and never reaches a client-level prompt —
and that is how a real defect survived this round's first green run: `prompt_key`
left `result` at whatever `prompt_check_move` returned, so every edit reported
`PROMPT_KEY_NOT_HANDLED` and the key was queued to the command queue as well.
Keys sent to the *outer* pane of the nested-client harness are the inner client's
terminal input, which does reach it, so cases 1491 (typing, backspace,
Escape-cancel) and 1492 (history recall and the `PROMPT_SINGLE` confirm prompt)
now drive the prompt the way a keyboard does and compare what it handed to the
command it was collecting for.

`src/extensions/ratatui_ui.rs` is the one piece with no upstream counterpart: it
floats the prompt as an overlay box instead of drawing it on the status row, so
`status_prompt_redraw` returns early and the terminal cursor stays hidden while
the overlay paints its own. It now reads the prompt object's buffer and index and
calls `prompt_replace_complete`; its richer candidate list (extension
subcommands, option names, layout names) moved into the extension itself, leaving
the ported prompt completing exactly what `prompt.c` completes.

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
from the reference. **It is now empty.** The last case, the `switch-mode`
command, closed with the Round-12 `prompt.c` / `window-switch.c` port and is
covered by `parity/cases/1488_switch_mode.sh` and
`parity/cases/1489_switch_mode_draw.sh`.

```sh
bash parity/run_known_gaps.sh   # "GAP" = still unported (expected); "CLOSED" = ported, promote it
```

The runner is an advisory tripwire — it exits non-zero only when a gap closes
(the feature got ported and its case should move to `parity/cases/`), so it never
reddens CI merely because the gaps still exist. With the directory empty it
exits 2 with `no cases in parity/known_gaps/*.sh`; the script is deliberately
left as-is rather than taught to treat "nothing to measure" as success. See
[`parity/known_gaps/README.md`](known_gaps/README.md) for the full inventory and
proof. These gaps do not count against the 1194/1194 ported surface; they measure
the unbuilt surface beyond it.

## Growing the suite

Add a `.fmt` (one format) or `.sh` (one scenario) file under `parity/cases/`.
Keep them small and single-purpose; number-prefix by category. The sibling
suites scaled this to thousands of cases — the same shape scales here.

To record a newly-found divergence that ztmux does *not* yet match, add a `.sh`
case under `parity/known_gaps/` instead and confirm it with `run_known_gaps.sh`.
