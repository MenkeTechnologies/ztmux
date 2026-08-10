# Bug Fixes

Fixes to the ztmux port, most recent first.

## Open

### Two client theme hooks are absent from the options table

- **Symptom:** `set-hook -g client-light-theme ...` fails with `invalid option`;
  next-3.7 accepts it. The same for `client-dark-theme`.
- **Root cause:** `vendor/tmux/options-table.c:1929`–`1930` declares both; the
  port's `OPTIONS_TABLE` has neither. Unlike the five pane hooks fixed on
  2026-08-09 these are not typos — there is no notify side either
  (`vendor/tmux/server-client.c:3089`/`:3092` call `notify_client` for them and
  the port has no counterpart), so this is an unported feature rather than a
  transcription slip.
- **Measurement:** `show-hooks -g` + `-gw` + `-gp` counts 68 names on the
  reference and 66 on the port; after the pane hook fix these two are the whole
  remaining difference.

### 27 upstream command flags have no counterpart in the port

- **Symptom:** eleven of the 92 commands reject flags the reference accepts —
  e.g. `break-pane -x` gives `command break-pane: unknown flag -x` where
  next-3.7 accepts it.
- **Measurement:** comparing the `getopt`-shaped argument template of every
  `cmd_entry` (the first member of `.args` in C, the first argument to
  `args_parse::new` in Rust) gives 564 flag slots across the 92 upstream
  commands, of which **27 are absent from the port**, 5 are port-only (the `o:`
  structured-output extension on the five `list-*` commands), and 1 differs in
  arity.
- **The eleven:** `break-pane` (`-W -x -X -y -Y`), `choose-buffer` (`-k -y`),
  `choose-client` (`-h -i -k -y`), `choose-tree` (`-h -k -y`),
  `command-prompt` (`-C -e -l -P`), `copy-mode` (`-S`), `customize-mode`
  (`-k -y`), `list-keys` (`-F -O -r`), `run-shell` (`-E -s`), `server-access`
  (`-g`). `refresh-client` declares `l::` where the C declares `l` — an
  optional-argument form for a flag the C reads with `args_has`
  (`cmd-refresh-client.c:263`), which is the one entry most likely to be a
  transcription artefact.
- **Largest block:** `break-pane`'s five are next-3.7's floating-pane geometry,
  dispatched in C to `cmd_break_pane_float` (`cmd-break-pane.c:50`), which the
  port does not have — one feature, not five flags.
- **Why nothing caught it:** no parity case passes any of the 27. Every one of
  the 564 slots is a mechanically derivable case, so the templates are a
  generator for the suite.

### Five rows are missing from the format table

- **Symptom:** `#{buffer_full}` expands to the empty string where next-3.7
  returns the buffer. Four commands reproduce it: `set-buffer hello` then
  `display-message -p '#{buffer_full}'` prints `hello` on the reference and
  nothing on the port.
- **Measurement:** `format_table[]` (`vendor/tmux/format.c:3203`) holds 195
  rows; `FORMAT_TABLE` (`src/ported/format.rs:3466`) holds 190. The five names
  the port does not have are `buffer_full`, `client_colours`, `client_theme`,
  `pane_pipe_pid` and `session_active`; every name the port does have is in the
  C's table, so these are absences rather than renames.
- **Why it is hard to see:** an unknown format variable expands to the empty
  string, exactly as a known variable with an empty value does, so expanding the
  name under both binaries proves nothing until the value is made non-empty.
- **Note:** `client_colours` is *referenced* by the port — 20 `dark-theme-*` /
  `light-theme-*` defaults in `src/ported/options_table.rs` (e.g. line 534)
  carry `#{?#{e|>=:#{client_colours},256},gray5,black}`, faithfully copied from
  `options-table.c:557`. So the port ships 20 default option values that expand
  a format variable its own table cannot resolve.
- **Why the suite does not have the case:** none of the five is named by any of
  the 1188 cases. Three are client-scoped and the harness attaches no client;
  one is a pid; `buffer_full` has no excuse at all.

### `#{history_bytes}` and `#{history_all_bytes}` use the wrong `sizeof`

- **Symptom:** on an idle 80x24 pane with three lines of output,
  `#{history_bytes}` is 1498 on the reference and 4964 on the port;
  `#{history_all_bytes}` is `24,960,80,400,6,138` against
  `24,960,80,3520,11,484`. Filling 20,000 lines gives 8,800,040 against
  71,200,040 — the port over-reports by about 8x.
- **Root cause:** the C multiplies the cell count by `sizeof *gl->celldata`
  (`struct grid_cell_entry`, `__packed`, 5 bytes) and the extended count by
  `sizeof *gl->extddata` (`struct grid_extd_entry`, `__packed`, 23 bytes) —
  `format.c:952`–`953` and `format.c:983`–`984`. The port multiplies both by
  `std::mem::size_of::<grid_cell>()` (44 bytes) in all four places:
  `src/ported/format.rs:1185`, `1186`, `1220`, `1222`. `grid_cell` is the
  *unpacked* cell struct with its `utf8_data`, not the storage entry.
- **Fix:** `size_of::<grid_cell_entry>()` and `size_of::<grid_extd_entry>()`.
  Note that the port's own entries are `#[repr(C)]` where the C's are
  `__packed`, so the corrected numbers will still be larger than the
  reference's — that difference is real memory, and RSS after 20,000 lines
  bears it out (10,432 KiB grown on the reference against 14,320 KiB on the
  port).
- **Why nothing caught it:** no parity case reads either variable; both are
  memory-shaped numbers, which the determinism rules discourage.

### Six read-only client gates are missing (seven sites)

- **Measurement:** `CLIENT_READONLY` appears at 24 sites in 9 files of the
  vendored C; `client_flag::READONLY` appears at 17 sites in 6 files of
  `src/ported`. The port also calls `proc_get_peer_uid` at 5 of the C's 7 call
  sites.
- **Absent gates:** `cmd-attach-session.c:111`–`117` and
  `cmd-switch-client.c:83`–`88` (a read-only client from a foreign uid may not
  clear its own read-only flag with `-r`; the port's `cmd_switch_client.rs:55`
  toggles unconditionally), `cmd-detach-client.c:73`–`78` (`detach-client -s` /
  `-a` / another client), `cmd-send-keys.c:178`–`181` (`send-keys` without
  `-X`), `window-copy.c:3723`–`3729` (any copy-mode command without
  `WINDOW_COPY_CMD_FLAG_READONLY`), `server-client.c:1573` (the bracketed-paste
  key path) and `server-client.c:2618` (the command-prompt result).
- **Why it matters:** these are the ACL surface of Chapter 38, and two of them
  are the uid checks that keep `server-access`-granted foreign clients from
  promoting themselves.

### The copy-mode command table has no argument templates and no read-only flag

- **Measurement:** `window_copy_cmd_table[]` (`vendor/tmux/window-copy.c:3118`)
  carries `.args` (an `args_parse` template) and `.flags` on every entry; 17 of
  the 95 entries have a non-empty template (`CP` on the fourteen copy/copy-pipe
  commands, `o` on `next-prompt`/`previous-prompt`, `e` on `scroll-to-mouse`)
  and 49 carry
  `WINDOW_COPY_CMD_FLAG_READONLY`. `WINDOW_COPY_CMD_TABLE`
  (`src/ported/window_copy.rs:3425`) has neither field — only `minargs` and
  `maxargs` — and `cs.wargs` does not exist, so `args_has(cs->wargs, 'P')` and
  friends have no counterpart.
- **Symptom:** `send-keys -X copy-line -P` suppresses the paste buffer on the
  reference (`list-buffers` returns nothing) and creates one on the port.
- **Related:** an unknown copy-mode command is silently ignored by the port —
  `send-keys -X scroll-to-mouse` returns nothing, while the reference *has* the
  command and takes the server down running it (`server exited unexpectedly`),
  which is why no parity case can drive it.

### The deferred request/reply mechanism is not ported

- **Measurement:** `input_csi_table[]` (`vendor/tmux/input.c:304`) has 43 rows;
  `INPUT_CSI_TABLE` (`src/ported/input.rs:257`) has 40. The three absent are
  `CSI ? Ps n` (`INPUT_CSI_DSR_PRIVATE`), `CSI Ps $ p` (`INPUT_CSI_QUERY`) and
  `CSI ? Ps $ p` (`INPUT_CSI_QUERY_PRIVATE`). Eighteen `input_*` functions have
  no counterpart under `src/`, and they are one cluster: `input_make_request`,
  `input_add_request`, `input_free_request`, `input_cancel_requests`,
  `input_request_reply`, `input_request_clipboard_reply`,
  `input_request_palette_reply`, `input_request_timer_callback`,
  `input_start_request_timer`, `input_send_reply`, `input_handle_decrqss`,
  `input_osc_52_parse`, `input_osc_52_reply`, `input_report_current_theme`,
  `input_start_ground_timer`, `input_ground_timer_callback`, `input_stop_utf8`
  and the C's four-argument `input_reply`.
- **Symptom (queried from inside a pane, reply read back off the tty):**
  `CSI ? 1004 $ p` returns `ESC [ ? 1004 ; 2 $ y` on the reference and nothing
  on the port; `DCS $ q m ST` returns `ESC P 0 $ r ESC \` on the reference and
  nothing on the port. `CSI 6 n` agrees (`ESC [ 1 ; 1 R`).
- **Semantics, not just rows:** the C's `input_reply(ictx, add, fmt, ...)`
  (`input.c:1153`) queues a reply behind any outstanding request when `add` is
  set, so replies stay ordered against clipboard and palette round-trips. The
  port's `input_reply_` (`src/ported/input.rs:1274`) writes straight to the
  bufferevent.

### A control-mode client never receives `%output`

- **Symptom:** attach a control client, then split a window whose command
  prints. The reference emits `%output %1 splitout`; the port emits the
  `%layout-change` line and nothing else. Reproduced with a four-line script on
  isolated sockets; `%begin`/`%end`/`%error`, `%session-changed`,
  `%window-add`, `%layout-change`, `%subscription-changed` and `%exit` all
  agree.
- **Localisation so far:** the call site (`src/ported/window.rs:1484`–`1488`
  against `vendor/tmux/window.c:1275`–`1278`), `control_write_output`
  (`control.rs:495` against `control.c:474`) and `control_add_pane`
  (`control.rs:253` against `control.c:247`) are all faithful, and the control
  client's `#{client_flags}` reads `attached,focused,control-mode` on both. The
  responsible line was not isolated; it is somewhere in the
  pending-block/flush chain or in the offset accounting behind
  `window_pane_get_new_data`.
- **Also worth fixing while there:** `control_append_data`
  (`src/ported/control.rs:707`) emits each byte with `*new_data.add(i) as char`,
  which re-encodes any byte over 0x7f as multi-byte UTF-8 where the C
  (`control.c:646`) copies the raw bytes.

### `-o json` and `-o tsv` are unusable under a non-UTF-8 locale

- **Symptom:** `LC_ALL=C ztmux list-windows -o json` emits `[_{"session":...`
  — the newline between array elements is an underscore, and the document is
  not parseable. All four formats collapse to a single line: json 4 lines to 1,
  jsonl 2 to 1, csv 3 to 1, tsv 3 to 1 with 38 underscores where the tabs were.
- **Root cause:** `Rows::render` (`src/extensions/structured.rs:117`) builds the
  whole document as one string and hands it to a single `cmdq_print`, whose own
  comment at line 116 says so. `server_client_print`
  (`vendor/tmux/server-client.c:3040`, ported faithfully) runs `utf8_sanitize`
  over a message when the client lacks `CLIENT_UTF8`, and `utf8_sanitize`
  (`utf8.c:784`, `src/ported/utf8.rs:770`) replaces every byte outside
  `0x20..0x7e` with `_`. tmux's own multi-line listings survive because they
  call `cmdq_print` once per line.
- **Blast radius:** running each of the 113 extension verbs against a fresh
  two-window server gives 95 producing output under the inherited UTF-8 locale
  and 31 under `LC_ALL=C` — 64 verbs flip from working to `parse [...]:
  expected value at line 1 column 2`. `parity/run_parity.sh:55` exports
  `LC_ALL=C LANG=C`, so the suite runs in exactly that locale, and no parity
  case can cover `-o` because it has no upstream counterpart.
- **Fix:** emit one `cmdq_print` per rendered line, as the ported `list-*`
  commands already do.

### `link=` in a style is still rejected

- **Symptom:** `set-option -g status-style 'bg=red,link=3'` fails with
  `invalid style:` under ztmux; next-3.7 accepts it and prints it back.
- **Root cause:** `style_parse` (`style.c:276`) puts the URI into a global
  hyperlink set and stores the small id in `sy->link`; `style_tostring`
  (`style.c:416`) reads it back through `style_link`. ztmux's `struct style` has
  no `link` field and no such set, so the directive has nowhere to go.
- **Scope:** unlike `width=`/`pad=`, this one is not just a field — it needs the
  `style_hyperlinks` store and `style_link`, and the id has to survive
  `style_copy`/`style_set`, which are whole-struct copies.
- **Found by:** the same style-directive check that turned up `width=`/`pad=`
  below.

## 2026-08-09

### Five pane hooks were misspelled in the options table (dead in both directions)

- **Symptom:** `set-hook -g pane-focus-in ...` failed with `invalid option:
  pane-focus-in` where next-3.7 accepts it. The same for `pane-focus-out`,
  `pane-mode-changed`, `pane-set-clipboard` and `pane-title-changed`. None of
  the five could fire either.
- **Root cause:** five of the seven `options_table_pane_hook!` rows in
  `src/ported/options_table.rs` dropped a `c`, against
  `vendor/tmux/options-table.c:1932`–`1938` — `pane-fous-in`, `pane-fous-out`,
  `pane-mode-hanged`, `pane-set-lipboard`, `pane-title-hanged`.
- **Dead inbound:** `options_match` (`options.c:678`) returned `None` for the
  real name, so `cmd_set_option.rs:124` answered with `invalid option: ...`.
- **Dead outbound:** the notify side was already correct — `window.rs:577`/`:584`
  fire `pane-focus-out`/`pane-focus-in`, `window_copy.rs:6432`/`:6525` fire
  `pane-set-clipboard`, `cmd_select_pane.rs:213` and three sites in `input.rs`
  fire `pane-title-changed`, `window.rs:1618`/`:1653` fire
  `pane-mode-changed` — so `notify_insert_hook`'s `options_get(oo, name)` looked
  up a name the table did not hold. Correcting the table connected thirteen call
  sites that were already in place.
- **Why nothing caught it:** the anti-drift gate compares function names, not
  string literals in macro arguments; the parity suite compares output, and a
  hook that never fires emits none; the row count is identical on both sides
  because a misspelled row is still a row. Only a name-level comparison finds it.
- **Pinned by** three tests in `options_table.rs`, each reading its expectations
  out of the vendored C or out of `src/ported` rather than a hand-typed list:
  every pane hook row is compared by name and in order against
  `OPTIONS_TABLE_PANE_HOOK` in `options-table.c`; every `notify_pane` name in
  `src/ported` must resolve to a registered pane hook; and each name must resolve
  through `options_match`, the same call `cmd_set_option` makes. Reverting only
  the five literals fails all three.

## 2026-07-31 (port round: `prompt.c` as an object, then `switch-mode`)

The last entry in `parity/known_gaps/` was the `switch-mode` command. It could
not be ported on its own: `window-switch.c` drives the prompt as an **object**
(`prompt_create` / `prompt_update` / `prompt_incremental_start` / `prompt_draw` /
`prompt_key` / `prompt_mouse` / `prompt_free`), and ztmux still carried the
pre-split design — nineteen `prompt_*` fields on `struct client` and twenty-four
`status_prompt_*` functions in `status.rs` taking a `*mut client`. So the round
is two ports: the prompt object first, then the mode that needs it.

- **`prompt.c` ported as `src/ported/prompt.rs`** — `struct prompt` owns the
  string, buffer, cursor index, `cmd_find_state`, callbacks, styles, cursor
  styles/colours, key mode, word separators, per-type history index, the `C-w`
  copy buffer and the completion list. `struct client` keeps a single
  `prompt: *mut prompt` in place of the nineteen fields, and `struct status_line`
  gains `prompt_cx` (`tmux.h:2014`) for the column `prompt_draw` writes back.
  The 13 `PROMPT_*` flags are all present now (ztmux had 5); `PROMPT_COMMANDMODE`
  replaces the separate `enum prompt_mode`, and `PROMPT_QUOTENEXT` (`C-v`),
  `PROMPT_BSPACE_EXIT`, `PROMPT_NOFREEZE`, `PROMPT_ACCEPT`, `PROMPT_ISMODE` and
  `PROMPT_EDITARROWS` are new behaviour rather than just new names.
- **The prompt draws itself into any screen.** `prompt_draw` takes a
  `prompt_draw_data` — a `screen_write_ctx`, a row, an x range and a
  cursor-column out-parameter — so the status line, a mode tree and switch mode
  all run the same editor. It expands `message-format` (with `#{message}`,
  `#{prompt_input}`, `#{prompt_flags}`, `#{prompt_type}` and `#{command_prompt}`
  set) instead of drawing the raw prompt string, which is why the `message-style`
  / `message-command-style` split and the `prompt-cursor-*` options now reach the
  prompt the way they do upstream. The cursor is the terminal's, positioned from
  `status_prompt_cursor`, not a reverse-video cell.
- **`prompt-history.c` ported as `src/ported/prompt_history.rs`** — the history
  lists moved out of `status.rs` under their real names (`prompt_load_history`,
  `prompt_save_history`, `prompt_up_history`, `prompt_down_history`,
  `prompt_add_history`) and gained the three accessors upstream added
  (`prompt_history_size`, `prompt_history_get`, `prompt_history_clear`).
  `clear-prompt-history` went through the accessor, which also fixed a leak: the
  old code freed the array but not the strings in it.
- **Completion is upstream's.** `prompt_complete_commands` /
  `prompt_complete_prefix` / `prompt_store_complete` / `prompt_draw_complete` /
  `prompt_mouse_complete` / `prompt_clear_complete` replace the session/window
  menu path — see the entry above.
- **`status.c` keeps the thin wrappers next-3.7 keeps** — `status_prompt_set`
  (which now builds a `status_prompt_data` bridge so the client-level callbacks
  still get their client), `_clear`, `_update`, `_redraw`, `_key`, `_cursor`,
  `_screen_line`, `_accept`, plus `status_message_area` (`status.c:413`) for the
  x/width the `message-style` width/align directives ask for.
- **`mode_tree_set_prompt` and friends** (`mode-tree.c:1068`–`1172`) — a mode
  tree now owns its prompt instead of borrowing the client's status prompt, and
  draws it on its own top or bottom row per `status-position`. `window-tree` and
  `window-customize` moved off `status_prompt_set` onto it, as upstream has them.
- **Callback ABI is upstream's.** `prompt_input_cb` returns
  `enum prompt_result` and is told `enum prompt_key_result` about the key that
  fired it, so a callback can distinguish an edit from a close from a cursor
  move. That is what lets `command-prompt -i` stay open across edits and
  `window_switch_prompt_callback` ignore anything but `PROMPT_KEY_HANDLED`.
- **`window-switch.c` ported as `src/ported/window_switch.rs`**, with
  `cmd_switch_mode_entry` (`cmd-choose-tree.c:87`, flags `F:kst:wZ`) registered in
  `cmd.rs` and the `Tab` / `BTab` prefix bindings from `key-bindings.c:405`.
  Those bindings open the picker in a scratch floating pane and pass `-k`, which
  needed `window_mode_entry.kill` (`window.c:1380`) and the `server_kill_pane`
  at the end of `window_pane_reset_mode` (`window.c:1428`) — neither existed
  here, so without them the scratch pane outlived the picker.
  `switch-mode-match-style` had been in the option table since the theme round
  with nothing reading it; `window-switch.c:289` is its reader, so it now takes
  effect on the columns `fuzzy_match` matched.
- **Pinned by:** `parity/cases/1488_switch_mode.sh` (the command surface: flag
  set, usage, entering and leaving the mode, the two bindings, the option),
  `parity/cases/1489_switch_mode_draw.sh` (the picker as drawn — list, selection,
  incremental prompt row, and the match style) and
  `parity/cases/1490_switch_mode_kill.sh` (`-k` disposing of the pane), the last
  two captured through a nested client the way 1484/1486 do.
  `parity/known_gaps/cmd_switch_mode.sh` is deleted; `parity/known_gaps/` now
  holds no cases.
- **The prompt's key path had no coverage at all**, which is how a real defect
  survived the first green run of this round: `prompt_key` left `result` at
  whatever `prompt_check_move` returned, so every edit reported
  `PROMPT_KEY_NOT_HANDLED` instead of `PROMPT_KEY_HANDLED` and the key was also
  queued to the command queue. The C resets it (`prompt.c:1151`). `send-keys`
  writes into a *pane* and never reaches a client-level prompt, so no case could
  drive it; the nested client can, because keys sent to the outer pane are the
  inner client's terminal input. `parity/cases/1491_prompt_keys.sh` (typing,
  backspace, Escape-cancel) and `parity/cases/1492_prompt_history_and_single.sh`
  (history recall, the de-duplicated history list, and the `PROMPT_SINGLE`
  confirm prompt on both a confirming and a non-confirming key) now cover it.
  They compare what the prompt handed to the command it was collecting for, not
  how it was drawn — ztmux floats the prompt as an overlay instead of taking the
  status row, which is an intended extension.

### `list-keys <key>` pads the flag column differently (open, unrelated)

- **Symptom:** `list-keys -T prefix c` prints `bind-key -T prefix c new-window`
  under ztmux and `bind-key  -T prefix c new-window` (two spaces) under next-3.7.
  Reproduces for every key, with or without the `switch-mode` bindings.
- **Scope:** a `cmd-list-keys.c` column-width matter, not a prompt one. Recorded
  here because `parity/cases/1488` had to take the two `switch-mode` bindings out
  of the full table rather than ask for them by key.

## 2026-07-31

### a style written as a format was cached unexpanded, so it never resolved

- **Symptom:** any style option whose value contains `#{…}` drew with the wrong
  cell. `#{E:mode-style}` produced an underline-colour escape (`ESC[58;5;0m`)
  instead of mode-style's fg/bg, and a style like
  `fg=#{?window_active,green,yellow}` drew with no colour at all. Because the
  result was cached, later `set-option`s to the *referenced* option (here
  `mode-style`) also stopped showing up.
- **Root cause:** `options_string_to_style` (`options.c:1010`) sets
  `o->cached = (strstr(s, "#{") == NULL)` — a style with no format in it parses
  to the same cell every time and is cached; one that has to be expanded against
  a format tree never is. The port had the test inverted
  (`cached = s.contains("#{")`), so exactly the styles that need expanding were
  marked cached, parsed once *literally* through `style_parse`, and returned from
  the cache forever after. Styles that could safely be cached were re-parsed on
  every draw instead.
- **Fix:** one negation at `options.rs:1239`, restoring the C's sense.
- **Found by:** porting `tree-mode-selection-style` (default `#{E:mode-style}`)
  and `tree-mode-preview-style` (default picks its colour from
  `#{?…pane_active…}`) — neither could have real behaviour with the cache
  inverted.
- **Pinned by:** `parity/cases/1486_tree_mode_draw.sh` (a
  `#{?window_active,…}` preview style) and
  `parity/cases/1487_tree_mode_selection_and_panes.sh` (the `#{E:mode-style}`
  default, including its following a later change to `mode-style`), both drawn
  through the choose-tree preview and compared cell for cell.

### `width=` and `pad=` in a style were rejected

- **Symptom:** `set-option -g status-style 'bg=red,width=10,pad=2'` failed with
  `invalid style:` and left the option unchanged; next-3.7 accepts both, and
  prints `width=50%` back quoted.
- **Root cause:** ztmux's `struct style` had neither field, so `style_parse`
  fell through to `attributes_fromstring` and errored. The directives are how a
  style carries a size: `pane-scrollbars-style` defaults to `width=1,pad=0`, and
  `status_message_area` (`status.c`) sizes the message from `message-style`'s
  width and align.
- **Fix:** `width`, `width_percentage` and `pad` on `struct style` with
  `STYLE_WIDTH_DEFAULT`/`STYLE_PAD_DEFAULT` of `-1`, the two parse arms
  including the `N%` form capped at 100, and the matching `style_tostring`
  output. `style_set`/`style_copy` are whole-struct copies so they carry the
  fields already.
- **Pinned by:** `parity/cases/1482_style_width_pad.sh`, plus three unit tests
  in `style.rs` covering round-tripping, the unset default, and the malformed
  forms that must be refused rather than clamped.

### `prompt_type` carried two types next-3.7 removed

- **Symptom:** `show-prompt-history -T target` printed `History for target:` and
  `clear-prompt-history -T target` silently succeeded. The vendored next-3.7
  rejects both with `invalid type: target`. Same for `window-target`.
- **Root cause:** ztmux's `prompt_type` (`src/lib.rs`) was the pre-split
  four-value enum — `command`, `search`, `target`, `window-target`, with
  `PROMPT_NTYPES = 4`. next-3.7 moved the prompt into `prompt.c` and cut the
  enum to two (`tmux.h:2061`, `PROMPT_NTYPES 2`), so anything else maps to
  `PROMPT_TYPE_INVALID`. The history arrays are sized on `PROMPT_NTYPES`, so the
  extra types also gave `show-prompt-history` two sections upstream does not
  have.
- **Fix:** the enum and `PROMPT_TYPE_STRINGS` are cut to `command` and `search`,
  which drops the two sections and makes the other names invalid. The branches
  that existed only to serve those types collapse to their remaining arm: the
  window-target completion menu, and the target-type entry into
  `status_prompt_complete` that skipped the `-t`/`-s` flag parsing.
- **Found by:** driving `show-prompt-history` / `clear-prompt-history` /
  `command-prompt -T` through `parity/verify_one.sh` against the reference while
  lining the prompt types up for the `struct prompt` port. No case covered the
  prompt types at all, which is why the suite was green over it.
- **Pinned by:** `parity/cases/1481_prompt_types.sh`.
- **Was still divergent, separately, now closed:** `prompt_complete`
  (`prompt.c:1538`) completes **commands only** and only at offset zero, showing
  matches as an inline underlined list rather than a menu. ztmux completed
  sessions and windows behind `-t`/`-s` and popped a menu. Closed by the
  `struct prompt` port in the round below, which brought the completion fields
  (`complete_list`/`complete_size`/`complete_display`) with it and deleted the
  menu path (`status_prompt_complete_list_menu`,
  `status_prompt_complete_window_menu`, `status_prompt_complete_session`,
  `status_prompt_menu_callback`).

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
next section — so the only one still open is `scroll-to-mouse`, which needs the
scrollbar-drag mouse state.

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
- **Still unported:** `scroll-to-mouse`. It drags the scrollbar slider. `wp->sb_slider_h`
  (`tmux.h:1301`) now exists — pane scrollbars are ported and drawn — but the drag
  itself also needs `tty.mouse_scrolling_flag` / `tty.mouse_slider_mpos`
  (`tmux.h:1769`) and the `KEYC_MOUSE_LOCATION_SCROLLBAR_*` key codes, and ztmux's
  `keyc` mouse table is the older six-location one (`PANE`, `STATUS`, `STATUS_LEFT`,
  `STATUS_RIGHT`, `STATUS_DEFAULT`, `BORDER`) with no scrollbar location to name.
  `server_client_check_mouse_in_pane` does compute the scrollbar geometry, but the
  location it resolves to binds to nothing.

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
- **Fix:** ztmux adopts a socket from `$TMUX` only when the path lives in its own
  `ztmux-<uid>` directory (tmux's is `tmux-<uid>`), and otherwise resolves
  through `make_label` (`default` under `ztmux-<uid>`), so nesting inside a real
  tmux pane can never put it on tmux's socket — `socket_from_environment`,
  `src/ported/tmux.rs`. Regression test: `ztmux_socket_adopts_only_its_own`. It
  still *exports* `$TMUX` to its panes, pointing at its own socket, because the
  ecosystem (powerline, tpm, prompts) detects a multiplexer by that variable
  being set; it deliberately introduces no `$ZTMUX` variable
  (`src/ported/environ.rs:301`).

### 2b. A nested command ran against the wrong server

- **Symptom:** inside a `-L pldbg` server, `ztmux -L pldbg run-shell "ztmux
  set-environment -g PROBE_VAR probe"` set the variable on the *default* socket;
  `ztmux -L pldbg show-environment -g PROBE_VAR` answered "unknown variable".
- **Root cause:** the fix for bug 2 above ignored `$TMUX` outright, so a command
  inheriting a pane's `$TMUX` fell through to the default socket instead of the
  server it was run from. Harmless on the default socket, wrong everywhere else.
- **Fix:** `$TMUX` is now adopted when it names a socket in ztmux's own
  `ztmux-<uid>` directory, which keeps a foreign tmux socket out (see above). A
  socket named with `-S` sits wherever the user put it and cannot be recognised
  this way, so a nested command there still needs its own `-S`.

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
