# Bug Fixes

Fixes to the ztmux port, most recent first.

## Open

Re-measured 2026-08-18 against the current build. Two entries that stood here
earlier the same day are gone because the defects are fixed, not because the
entries were tidied away: the `choose-tree` `i` info view (ported, pinned by case
1510) and `link=` in a style (ported, pinned by cases 1554/1555). One entry
shrank to a smaller, sharper claim after live probing, and one new one was added.

### One upstream command flag has an arity the port gets wrong

- **Symptom:** `refresh-client -lZ` gives `no current client` under ztmux and
  `command refresh-client: unknown flag -Z` upstream (both observed today).
- **Root cause:** the port declares `l::` where the C declares `l`
  (`cmd-refresh-client.c:39`), so `-l` swallows the next character as its value.
  Underneath, `cmd_refresh_client.rs:203` still implements the pre-next-3.7
  `-l [target-pane]` semantics with `clipboard_panes` and a `CLIPBOARDBUFFER`
  client flag; next-3.7 (`cmd-refresh-client.c:263`) is just
  `tty_clipboard_query(&tc->tty)`, and `CLIENT_CLIPBOARDBUFFER` exists nowhere in
  `vendor/tmux`.
- **Why it is not a one-line fix:** dropping the argument makes the port's own
  OSC 52 delivery path (`tty_keys.rs:1814-1828`) unreachable, because nothing
  else registers a pane. Upstream delivers that reply through
  `input_request_clipboard_reply`, which is part of the unported request/reply
  mechanism below. The two have to land together or `refresh-client -l` becomes a
  query whose answer goes nowhere.
- **`copy-mode -S` is no longer absent.** It was the one genuinely missing flag;
  it landed with the scrollbar mouse locations and is accepted now (rc 0 on both).

### Four read-only client gates remain (of seven sites)

- **Re-measured 2026-08-18.** `CLIENT_READONLY` has 24 sites across 9 files in
  the C; `client_flag::READONLY` has 19 across 8 in `src/ported`
  (`cmd_detach_client.rs` is the file with none).
- **Landed 2026-08-18:** `copy-mode` gained `CMD_READONLY`
  (`cmd-copy-mode.c:39`) so a read-only client can enter copy mode at all, and
  `send-keys` gained both the flag and the exec gate together
  (`cmd-send-keys.c:42-43`, `:178-181`). Those had to be one change: the flag
  without the gate would have turned `send-keys` into an unauthenticated
  key-injection channel rather than closing one. Verified: a read-only client is
  refused, nothing reaches the pane, and copy mode still opens.
- **Still missing:** the `proc_get_peer_uid` gates on `attach-session:112` and
  `switch-client`, plus the `cmd_detach_client.rs` sites. These are
  unobservable on a single-uid machine and need a second real account to
  exercise, which is why they are recorded rather than claimed either way.

### DECRQSS and the queued request/reply mechanism are not ported

- **Narrowed 2026-08-18.** This entry used to claim three missing
  `input_csi_table[]` rows and a dead `CSI ? Ps n`. Both are wrong now: the table
  has **43 rows on both sides**, `INPUT_CSI_DSR_PRIVATE`, `INPUT_CSI_QUERY` and
  `INPUT_CSI_QUERY_PRIVATE` all exist, and `CSI ? 1004 $ p` answers
  `ESC [ ? 1004 ; 2 $ y` on **both** binaries (probed from inside a pane, reply
  read back off the pty).
- **What is still missing, probed the same way:** `DCS $ q m ST` returns
  `ESC P 0 $ r ESC \` on the reference and **nothing** on the port —
  `input_handle_decrqss` has no counterpart.
- **And the machinery behind it:** fifteen `input_*` functions have no
  counterpart under `src/`, all one cluster — `input_make_request`,
  `input_add_request`, `input_free_request`, `input_cancel_requests`,
  `input_request_reply`, `input_request_clipboard_reply`,
  `input_request_palette_reply`, `input_request_timer_callback`,
  `input_start_request_timer`, `input_send_reply`, `input_handle_decrqss`,
  `input_osc_52_parse`, `input_osc_52_reply`, `input_start_ground_timer` and
  `input_ground_timer_callback`.
- **Semantics, not just rows:** the C's `input_reply(ictx, add, fmt, ...)`
  (`input.c:1153`) queues a reply behind any outstanding request when `add` is
  set, so replies stay ordered against clipboard and palette round-trips. The
  port's `input_reply_` (`src/ported/input.rs:1274`) writes straight to the
  bufferevent. This is the same cluster the `refresh-client -l` entry above is
  blocked on.

### `dim=` in a style is rejected

- **Found 2026-08-18** by sweeping every style directive through both binaries.
  It is the only one that still differs: `set-option -g status-style 'dim=30'`
  fails with `invalid style:` under ztmux and is accepted upstream.
- **Root cause:** `struct style` has no `dim` field, and the value is not just
  stored — `tty_attributes` dims the resolved fg and bg through `colour_dim`
  (`tty.c:2650-2658`), reading it from `tty_style_ctx.dim`.
- **Scope, and why it is not parse-only:** this port has no `tty_style_ctx` at
  all; `tty_attributes`, `tty_cell`, `tty_draw_line` and
  `tty_default_attributes` take `defaults`/`palette`/`hyperlinks` as separate
  parameters. Porting `dim=` means introducing the struct and threading it
  through all of them plus `screen_write` and `screen_redraw`, and adding
  `colour_dim`. Accepting the directive without that would store a value nothing
  reads — a config that looks applied and renders undimmed — so it stays
  rejected until the render lands.
- Every other directive now agrees, including the three fixed today
  (`set-default`, `link=`, `nolink`); the full accept/reject set is pinned by
  case 1554.

### `-o json` and `-o tsv` are unusable under a non-UTF-8 locale

- **Symptom:** `LC_ALL=C ztmux list-windows -o json` emits `[_{"session":...`
  — the newline between array elements is an underscore, and the document is
  not parseable. All four formats collapse to a single line: json 4 lines to 1,
  jsonl 2 to 1, csv 3 to 1, tsv 3 to 1 with 38 underscores where the tabs were.
  Still reproduces today.
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
  commands already do. Five call sites
  (`cmd_list_buffers/clients/panes/sessions/windows.rs`) share one
  `cmdq_print!(item, "{}", out.render())`, so the change belongs behind one
  shared helper rather than five edits.

### `TTY_WAITBG`/`TTY_WAITFG` and `tty_repeat_requests`' `force` are unported

- **Found 2026-08-18** while un-inverting the start-timer condition
  (`tty.c:318`), recorded rather than absorbed into that change.
- `tty_send_requests` sets `tty->flags |= (TTY_WAITBG|TTY_WAITFG)` after the OSC
  10/11 queries (`tty.c:409`) and the start-timer callback clears them
  (`tty.c:322`). Neither flag exists in this tree — the only mention is a comment
  at `src/ported/tty.rs:445` recording the gap — so both sites are absent along
  with every consumer.
- `tty_repeat_requests` takes an `int force` parameter (`tty.c:414`); the port's
  takes none.
- **Not known to be observable.** No probe has been constructed that
  distinguishes the two binaries on this, so it is filed as unported surface
  rather than as a reproduced defect.

## 2026-08-18 (client-render round: what only an attached client could see)

Thirty-two parity cases were written over the surface a live session touches —
splits and borders, copy mode, the status bar, attach/resize, mouse and
scrollbars, menus and popups, wide characters, hooks. None of it had been
reachable before, because those paths only run for an ATTACHED CLIENT and until
this round no case could see one. Every client-level case written found something.

### `variation-selector-always-wide` never fired, and the naive fix would have broken the other direction

- **Symptom:** every VS16 emoji rendered ONE COLUMN NARROW, shifting the rest of
  the line — so wrapping, cursor position and `capture-pane` all desynchronised
  after one.
- **Root cause:** `utf8_is_vs` compared against `EF BF 8F` where the C uses
  `EF B8 8F` (`utf8-combined.c:73`). One byte. Real U+FE0F never matched.
- **The second bug it was hiding:** `screen_write_combine` set `force_wide`
  unconditionally where the C gates it on the option (`screen-write.c:2824`), so
  fixing the constant alone would have made every VS16 emoji wide even with the
  option off. Both fixed together; heart and warning now measure 2 with the
  option on and 1 with it off, matching.
- **Two unit tests were pinning the typo as correct**, one named
  `is_vs_matches_port_constant`, with a comment explaining the divergence from
  `vendor/tmux`. A test that encodes a bug as expected hides it from the process
  meant to catch it. Renamed and inverted to assert the C.

### `display-menu` drew a rule per empty argument, and `-b` was discarded

- `menu_add_item` has two guards (`menu.c:77-78`); the port had only the first,
  so consecutive separators were not collapsed and any menu with two blank
  arguments showed a doubled line.
- `-b` was parsed, validated, then thrown away: `lines` was immutable and never
  took the resolved choice (`cmd-display-menu.c:359`), so `-b double` drew
  single-line glyphs while the `menu-border-lines` OPTION path worked.

### Copy-mode line numbers went stale on every vertical cursor move

With line numbers active the C repaints the whole screen when the cursor changes
row (`window-copy.c:5437-5443`), because the gutter depends on the row. The port
did not, so the relative numbers kept counting from the old row and the yellow
current-line style stayed on it.

### `set -p` wrote the window option for the three pane-scoped border options

`options-table.c` gives `pane-active-border-style`, `pane-border-lines` and
`pane-border-style` the scope `OPTIONS_TABLE_WINDOW|OPTIONS_TABLE_PANE`; the
port's table had plain `OPTIONS_TABLE_WINDOW` for exactly those three.
`options_scope_from_name` only consults `-p` under the combined
`case WINDOW|PANE:` label (`options.c:900`), so for those three it fell through
to the window branch: the command succeeded and changed the border on every pane
in the window instead of the one named. Nothing errored — that is what let it
survive. The scope resolution itself was a faithful port; the whole defect was
three characters of table drift, which is the argument for checking the table
mechanically rather than by eye. All 180 entries now agree with the C on name,
scope and type.

### Two `#{hook_*}` formats were wrong, and neither could fail loudly

- `#{hook_pane}` rendered `%%3` instead of `%3`. C `notify.c:212` formats with
  printf, where `%%` is one literal percent; the port's `format_add!` takes a
  RUST format string, where `%%` is two. Any hook passing `#{hook_pane}` to
  another command handed it a target that cannot resolve.
- `#{hook_window}` and `#{hook_window_name}` were EMPTY for every pane-scoped
  hook. The C sets them twice — once from `w`, again from `wp->window` on the
  pane branch (`notify.c:213-215`) — because a pane notification passes no
  window. `pane-died`, `pane-exited`, `pane-mode-changed` and
  `pane-title-changed` all reported a pane with no window it belonged to.

### OSC 8 hyperlinks were off in every shipped build

The `Hls` capability sat behind a cargo feature (`hyperlinks`) that is not in
`default`, so `tty_feature_hyperlinks` had an empty capability list and
hyperlinks were dropped entirely: a pane printing one came back as bare text and
`#[link=...]` drew nothing. Upstream gates it on an ncurses-version `#if`
(`tty-features.c:98`) asking whether the local ncurses can express an extended
capability name — a question ztmux, which links no ncurses and emits the string
from its own table, has nothing to answer.

Four separate defects sat behind that gate, all of them invisible while it was
closed:

| Defect | C reference | Effect |
| --- | --- | --- |
| `link=` / `nolink` rejected by `style_parse` | `style.c:246`, `:276-284` | a style option is all-or-nothing, so the whole option was lost |
| `set-default` missing from the enum, parse, tostring and format_draw | `style.c:112`, `format-draw.c:865-870` | rejected outright; it moves the BASE, so a later `pop-default` lands on the new cell |
| `style_tostring`'s `pad=` block did not advance `off` | `style.c:411` | correct only while `pad=` was LAST; `pad=2,link=…` would have had the link overwrite it |
| `grid_string_cells_code` took `has_link` by value | `grid.c:1166` | `capture-pane -e` emitted the opening OSC 8 and never closed it |

The `has_link` one is why the nested-client cases could not see the bug from
outside: the C threads `int *has_link` ACROSS cells, so the closing sequence is
only written when a link was open on an earlier one. As a per-call local
re-initialised to false, the `else if` could never fire — and the ztmux acting as
the OUTER server was mis-serialising its own grid.

### The terminal-feature table had drifted, including three whole terminals

`tty_feature_progressbar` did not exist, so `Spb` was never applied and the OSC
9;4 progress bar never emitted despite `screen_set_progress_bar` being ported.
The `tmux` entry was missing `extkeys` and `progressbar`; iTerm2 and mintty were
missing entries of their own; **foot, WezTerm and ghostty were absent outright**,
so a user on any of those three got no feature detection at all. All 21 features
and all 8 terminals now match `tty-features.c` entry for entry.

Five unit tests were pinning the drifted values — one asserted the `tmux` row
WITHOUT `extkeys` and `progressbar`. Corrected against strings derived
mechanically from the C table and cross-checked against what the reference prints
for `#{client_termfeatures}`.

### A shared static made the unit suite flaky about once in ten runs

`key_string_lookup_key` returns a pointer into a function-level buffer, faithful
to the C's `static char out[64]`. The tmux server is single-threaded so that is
fine in production — but under the parallel test runner it made every test that
renders a key race every other caller in the binary, surfacing as `"\0C-?"` and
`"\0-Space"`: a half-overwritten buffer. A mutex in the test helper could not fix
it, because the racers were production call sites outside the helper. The buffer
is thread-local now, which is indistinguishable for the running server and cannot
be clobbered across threads. Eighteen consecutive clean full-suite runs since.

### Three stale bug notes in `style.rs` described defects that were already fixed

The test module's header listed two "latent bugs in this port" — a
`style_tostring` truncation of any multi-field style, and a `range=user` render
that segfaulted. Both had been fixed; the tests already asserted the correct
behaviour while their names (`bug_tostring_multifield_truncated`,
`bug_tostring_user_range_crashes`) and comments still claimed otherwise. Renamed
and rewritten to record what the bugs were rather than assert they are current.

## 2026-08-18 (Open-section audit)

Six entries under `## Open` described defects that no longer reproduced. Each was
re-tested against the current build before being moved here; none was deleted.
The list had been mis-steering triage — an earlier round this same day ranked work
partly off entries that were already closed.

### Verified fixed, with the evidence used

| Entry | Verdict | Evidence |
| --- | --- | --- |
| `mode_tree_draw` row composition | fixed 2026-08-18 | This session's port; four mode screens byte-identical, case 1508 |
| Two client theme hooks absent | fixed `3839310b6f` | `{show-hooks -g; -gw; -gp} \| wc -l` = **68 on both**; `set-hook -g client-light-theme` accepted |
| Five format table rows missing | fixed `212038da0a` | 195 names on both, `comm -23`/`comm -13` both empty; `set-buffer hello; display -p '#{buffer_full}'` -> `hello` on both |
| `#{history_bytes}` wrong `sizeof` | fixed `212038da0a` | idle 80x24: **960 on both**; after 2000 lines: 280040 on both |
| copy-mode table lacked arg templates | fixed `35a2816d69` + `73dc53ce53` | table now carries templates and the read-only flag |
| control-mode client never got `%output` | fixed `212038da0a` | transcripts byte-identical (`event_loop/bufev.rs`) |

The remaining pacing difference in control mode (the C throttles to roughly
3.5 KB/s, the port does not) delivers a correct, in-order superset rather than a
protocol break, and is deliberately not filed as a defect.

### What replaced them

`27 upstream command flags` was not stale but **wrong**, and is re-measured above:
the true absent count is 1. The read-only entry was over-counted and is corrected
to 4 of 7. Three genuinely-open items were added that nothing had recorded: the
`choose-tree` `i` info view, the `refresh-client -l` arity and semantics, and the
`TTY_WAITBG`/`TTY_WAITFG` drift.

## 2026-08-18 (in-pane prompt)

### `command-prompt -P` was unported, so every copy-mode prompt sat on the status line

- **Symptom:** pressing `?`, `/` or `:` in copy mode drew the prompt on the status
  row, replacing the status bar. next-3.7 draws it inside the pane on the row
  above and leaves the status bar visible. Same for `g` (goto line) and the
  jump keys, and for any user config reaching them — Hashrocket's `dotmatrix`
  binds `prefix e` to `copy-mode \; send-keys "?Error" C-m`, which lands here.
- **Root cause:** the flag existed in `prompt_flags` with nothing behind it.
  `struct window_pane` had no `prompt` / `prompt_data` / `prompt_cx`, none of
  `window.c`'s five pane-prompt functions were ported, and `cmd-command-prompt`
  did not accept `-P`. The default binding table had then been written *without*
  the flag to match what the port could do, which made the whole thing
  self-consistent: nothing looked broken, and no state-level check could see it.
- **Fix:** ported `struct window_pane_prompt` and its two callbacks,
  `window_pane_set_prompt` / `_clear_prompt` / `_has_prompt` / `_update_prompt` /
  `_prompt_key` (`window.c:82`, `:1442`–`:1580`); added the three `window_pane`
  fields; added `-P` to `cmd-command-prompt` with the pane-vs-status dispatch and
  the multi-prompt update branch (`cmd-command-prompt.c:95`, `:179`, `:218`); the
  key routing that prefers the active pane and falls back to the first visible
  pane holding a prompt (`server-client.c:1650`); and `redraw_draw_pane_prompt`
  (`screen-redraw.c:1525`, called at `:1677`). Restored `-P` on all 32 default
  bindings.
- **Verified:** through a real client, both binaries now draw `(search up)` on
  row 23 and keep the status bar on row 24, with the same search outcome. The
  known gap reports `CLOSED`; promoted to case **1506**, rendering pinned by case
  **1507**, and the 32 keys deleted from case 1498's exclusion list so they are
  blocking again.

### Tagged rows in `choose-tree` used a bright attribute instead of the theme colour

- **Symptom:** a tagged row in `choose-tree` / `choose-buffer` was drawn bold
  where next-3.7 draws it in the theme's cyan.
- **Fix:** ported `enum colour_theme` and applied
  `COLOUR_THEME_CYAN|COLOUR_FLAG_THEME` to `gc`/`gc0` before the row is drawn,
  restoring the saved foregrounds after — the save/restore shape the C uses
  (`mode-tree.c:844`, `:931`, `:969`) in place of the port's bright-attribute
  toggle. Verified through a client: the tagged row now carries the same
  `38;2;95;158;160` the reference emits.
- **Not claimed as closed:** the surrounding row *composition* in
  `mode_tree_draw` is still at an older revision — next-3.7 splits the row into a
  `MODE_TREE_PREFIX_FORMAT` prefix drawn with `format_draw` plus a styled
  separator, and the port draws a single composed string. So the tagged
  highlight is now faithful while `choose-tree` rendering as a whole is not.
  Recorded as open below rather than presented as a finished port.

## 2026-08-18 (Hashrocket dotmatrix acceptance round)

Found by loading [hashrocket/dotmatrix](https://github.com/hashrocket/dotmatrix)'s
`.tmux.conf` into both binaries and diffing the resulting state, then driving the
config through a real client (a second server nested in a pane of the first).
Every option and every binding the config sets was already identical, and the
whole default binding table diverged only at the entries already listed in case
1498's exclusions. The two bugs below were in the layers under the config: the
first in what a client draws, the second surfaced while building the nested
client — ztmux accepted `attach -t <window-name>` where tmux refuses it.

### Theme colours never reached the terminal, so the default status bar was unstyled

- **Symptom:** the whole status line painted with the terminal's default
  colours instead of next-3.7's green. Loading dotmatrix's
  `status-left '#[fg=colour235,bg=colour76,bold] #S '`, the reference paints the
  `#S` segment AND carries the status style into the window list; the port
  painted the `#S` segment and then reset:

  ```
  tmux   ^[[1m^[[38;5;235m^[[48;5;76m hr ^[[0;4m^[[38;2;13;13;13m^[[48;2;154;205;50m0:one*…
  ztmux  ^[[1m^[[38;5;235m^[[48;5;76m hr ^[[0;4m0:one*…
  ```

- **Root cause:** next-3.7's `status-style` defaults to
  `bg=themegreen,fg=themeblack`, theme colours that carry `COLOUR_FLAG_THEME` and
  resolve to a real colour only at render time. `colour_fromstring`,
  `colour_tostring` and `server_client_update_theme_colours` were all ported —
  which is why `show-options -g status-style` was already byte-identical — but
  `tty_map_theme_colour` (`vendor/tmux/tty.c:2800`), the function that turns the
  flag into the client's resolved colour, had no counterpart. A flagged colour
  therefore reached `tty_colours_fg`/`_bg` still carrying `0x04000000`, matched
  neither the RGB nor the 256 branch, and was written as default.
- **Fix:** ported `tty_map_theme_colour` and called it where the C does — in
  `tty_check_fg` (`tty.c:2843`), `tty_check_bg` (`:2904`), `tty_check_us`
  (`:2955`) and `tty_force_cursor_colour` (`:757`). Two grid-side sites were
  missing the same flag: `grid_string_cells_fg`/`_bg`/`_us` (`grid.c:866`/`:922`/
  `:978`), which is what `capture-pane -e` serialises, and `grid_clear_cell`
  (`grid.c:281`), where a theme background needs the extended cell an 8-bit
  `data.bg` cannot hold.
- **Why nothing caught it:** every existing case asks the server what it
  *stored*. A theme colour stores and prints back perfectly; only a client
  drawing to a terminal shows that it never resolves, and the parity harness has
  no terminal. Case **1504** builds one — a second server inside a pane of the
  first, with a client attached to it — so `capture-pane -e` on the outer server
  re-serialises the attributes the inner client actually emitted. Mutation-tested:
  reverting the fix turns it red.

### `attach-session` resolved a window name as if it were a session

- **Symptom:** with a session `hr` whose windows are `code` and `docs`,
  `attach -t code` is `can't find session: code` on the reference and was
  accepted by the port (failing later, only because the test ran without a
  terminal). Shell aliases of the `tmux attach -t <name>` shape would attach to
  something other than what they named.
- **Root cause:** `cmd-attach-session.c:80` picks the target *type* with
  `tflag[strcspn(tflag, ":.")] != '\0'` — strcspn stops at the first `:` or `.`,
  so the test is "the target CONTAINS one of them", i.e. a window/pane target.
  The port had `!tflag.trim_start_matches([':', '.']).is_empty()`, which strips
  *leading* separators and asks whether anything remains — true for every
  ordinary name. So `code` was resolved as `CMD_FIND_PANE`, which matches window
  names, and `:`/`.` alone took the session branch the C sends to the pane one.
  The flag argument went with it: plain session targets lost
  `CMD_FIND_PREFER_UNATTACHED`.
- **Fix:** `tflag.contains([':', '.'])`, the C's test. The sibling site
  (`cmd-switch-client.c:69`, `":.%"`) was already faithful — it kept the raw
  `strcspn` — so this was the one place the pointer-to-`Option<&str>` change lost
  the meaning.
- **Pinned by:** case **1505**, which runs eleven target shapes through
  `attach-session` and reads the error as the marker for accepted vs rejected.
  Mutation-tested.

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
