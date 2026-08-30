# Bug Fixes

Fixes to the ztmux port, most recent first.

## Open

Re-measured 2026-08-21. Four entries that stood here this morning are gone
because the defects are fixed, not because they were tidied away: structured
output under a non-UTF-8 client, the `refresh-client -l` arity, the queued
request/reply mechanism (DECRQSS with it), and `TTY_WAITBG`/`TTY_WAITFG` with
`tty_repeat_requests`' `force`. The read-only client gates went with them. All
five are written up below with what pins them.

What stands here now was found while doing that work.

Three entries that stood here earlier on 2026-08-18 are gone for the same
reason — the defects are fixed: the `choose-tree` `i` info view (ported, pinned by case
1510), `link=` in a style (ported, pinned by cases 1554/1555), and `dim=` in a
style, which was added to this list and then closed the same day once
`tty_style_ctx` came over (case 1556). One entry shrank to a smaller, sharper
claim after live probing.

Every style directive now agrees with the reference: 53 of them, swept through
both binaries, zero divergences.

### Two tty flags the C sets are still absent

- **Found 2026-08-21** while porting the request queue, which needed the flag
  word next to them.
- `TTY_WINSIZEQUERY` (`tmux.h:1754`) guards the winsize query
  (`tty.c:146-149`) and is cleared when the reply lands
  (`tty-keys.c:689`, `:735`). Neither the flag nor the query exists here.
- `TTY_BRACKETPASTE` (`tmux.h:1757`) is set and cleared as a pane turns
  bracketed paste on and off (`tty-keys.c:645-647`) and makes a partial paste
  end extend the key delay (`tty-keys.c:973`). The port has the pane-side
  `MODE_BRACKETPASTE` but not the tty-side flag, so that delay never applies.
- **Not known to be observable.** Both are recorded as unported surface rather
  than as reproduced defects; the bit positions are left free in `tty_flags`
  with a comment naming what belongs there.

## 2026-08-30 (floating panes, and a client flag that was never there)

Continuing the flag audit into next-3.7's newest surface: `new-pane`,
`move-pane`'s floating operations, and `refresh-client` against a real client
(cases 1946–1953).

### The `no-detach-on-destroy` client flag did not exist

- **Found 2026-08-30** by `parity/cases/1952_refresh_client_flags_and_panning.sh`,
  which sets each client flag that applies to a non-control client and reads
  `#{client_flags}` back.
- `CLIENT_NO_DETACH_ON_DESTROY` (`tmux.h:2248`) was absent in four places: the
  bit itself, the `no-detach-on-destroy` name in `server_client_set_flags`
  (`server-client.c:2875`), its label in `server_client_get_flags` (`:2909`),
  and the `cs_new` fallback in `server_destroy_session` (`server-fn.c:456-470`)
  that is the whole point of the flag — with `detach-on-destroy` on, killing a
  client's session normally detaches it, and a client carrying this flag is
  moved to another session instead.
- All four ported. `parity/cases/1953_no_detach_on_destroy_client_flag.sh`
  watches a flagged client's session get killed and the client come back on the
  surviving session rather than exiting.

## 2026-08-30 (split-window and join-pane: what the flag audit turned up)

The round began by diffing every `.args` string in `vendor/tmux/cmd-*.c` against
the whole case corpus, which named 107 flag letters no case had ever passed.
Cases written for the first of them found four defects.

### `split-window`/`new-pane` had drifted from next-3.7 wholesale

- **Found 2026-08-30** by `parity/cases/1941_split_window_empty_and_border_flags.sh`.
- `cmd_split_window_exec` was a pre-`layout_get_tiled_cell` shape: it computed
  the split size itself and stopped at `spawn_pane`. Everything the C does on
  either side of that was missing — `-E`'s empty pane (only `-I` set
  `SPAWN_EMPTY`), the `command cannot be given for empty pane` refusal
  (`cmd-split-window.c:110-116`), `-B`'s `pane-border-lines` choice, and the
  whole post-spawn block that puts `-s`, `-S`, `-R`, `-B`, `-k`, `-m` and `-T` on
  the new pane's own options (`:173-217`). `split-window -E 'sleep 300'` created
  a pane instead of failing; `-S fg=red` was accepted and dropped.
- Fixed by porting the exec against the C, plus the two functions it leans on:
  `layout_get_tiled_cell` (`layout.c:1593`) and `options_search`
  (`options.c:666`). The C's `-W` block is still absent — it needs
  `window_pane->wait_item` and `window_pane_wait_finish`, neither ported — and
  that is now stated in the code rather than silently missing.

### `join-pane -p` could not take a percentage

- **Found 2026-08-30** by `parity/cases/1931_join_pane_percentage_size.sh`.
- The `'p'` branch read the value of flag `l`: `args_strtonum_and_expand(args,
  b'l', ...)`. With no `-l` given that returns `missing`, so every
  `join-pane -p N` failed with `size missing` instead of sizing the new pane.
- Fixed by routing join-pane through `layout_get_tiled_cell` the way
  `cmd-join-pane.c:419` does, which owns the `-l`/`-p`/`-b`/`-f` reading. That
  also **closed the recorded gap** `join_pane_before_placement.sh`: the C leaves
  `cmd_join_pane_exec`'s own `flags` at zero (`cmd-join-pane.c:379`), so `-b`
  reaches the layout cell but never the pane-list insert, which is exactly the
  side-swap the gap recorded. Promoted to
  `parity/cases/1943_join_pane_before_placement.sh`.

### `remain-on-exit` was missing next-3.7's fourth choice, and `-k` crashed the server

- **Found 2026-08-30** by `parity/cases/1944_split_window_title_and_remain_flags.sh`,
  which was written to pin the newly-ported `-k`/`-m`/`-T` block above.
- next-3.7 added `"key"` to `options_table_remain_on_exit_list`
  (`options-table.c:93-95`); this tree carried the older three-name list. The C's
  `-k` writes 3 into that option, so the first `split-window -k` took the server
  down with `index out of bounds: the len is 3 but the index is 3` in
  `options_value_to_string` as soon as anything read the option back.
- Two readers were missing with it: `server_destroy_pane`'s switch had no
  `case 3` (`server-fn.c:347`), so a pane under `key` would have been destroyed
  rather than kept; and the block that dismisses such a pane on the next key,
  setting the option back to `off` (`server-client.c:1557-1566`), did not exist.
- All three ported. The C's `KEYC_IS_PASTE` half of that condition has no
  counterpart — this port has no bracketed-paste key encoding at all, so no key
  it can receive is a paste key; the code says so.
- Pinned by `parity/cases/1945_remain_on_exit_key_choice.sh`, which sets each of
  the four choices, and watches a pane exit under `key`.

### `verify_one.sh` reported OK for a case file that does not exist

- **Found 2026-08-30**, and it is a measurement defect rather than a port one, so
  it is recorded here and fixed on its own.
- The script takes a PATH (`parity/verify_one.sh parity/cases/NAME.sh`). Given a
  bare basename it runs `bash NAME.sh` for both binaries, both fail identically
  with `No such file`, the byte comparison matches, and it prints `OK`. Every
  bare-name check reads as a pass no matter what the case says.
- The three divergences above were all "verified" that way before the full suite
  caught them, which is what exposed it.

## 2026-08-29 (coverage round: the surface no case had touched)

Three hundred and fifty-nine parity cases (1566–1925) were written against what the suite did not
measure rather than against what it already did: `wait-for`, the config
conditions (`%if`/`%elif`/`%else`/`%endif`/`%hidden`) and config variable
expansion, `source-file`'s `-q`/`-n`/`-v`, `show-options -A`/`-v`/`-q` and its
scope flags, command-name resolution (unknown, ambiguous, abbreviated),
`command-alias`, `bind`/`unbind`/`list-keys` flags and key tables, `run-shell`,
`pipe-pane`, and 48 format variables that had never appeared in a case (94 of the
195 in `format.c`'s table had not), then the buffer file commands, `send-keys`,
`new-session`'s creation flags, `capture-pane`'s output flags, the layout and
zoom commands, the "no current client" path of the client-only commands, the
target-token syntax, the option arrays, and the session- and window-level
commands, the hooks, and the option scope chain -- and then, once every table
was name-covered, the tables' contents: usage strings, the input parser, the
modifiers that need a client, and the options whose behaviour nothing asserted.
Ten defects came out of it.

### `#{window_linked_sessions}` counted winlinks, not sessions

- **What it printed:** the number of winlinks the window has. A window linked
  twice into the same session read `2` where tmux reads `1`; the accompanying
  `#{window_linked_sessions_list}`, which does walk winlinks, printed two entries
  either way, so the two formats disagreed about the same window.
- **Cause:** `format_cb_window_linked_sessions` returned
  `(*window).references` — the older tmux implementation. next-3.7
  (`format.c:2919`) counts sessions: one per session group that holds the window
  (its first session standing for the group) plus each ungrouped session that
  holds it, via `winlink_find_by_window`.
- **Fix:** ported as the C has it, including the session-group half, which the
  reference count needs and a winlink count cannot express.
- **Pinned by:** case 1641, which builds a window in two sessions with three
  winlinks — the one shape where the winlink count and the session count differ —
  plus 1642/1643/1644 for the single-session values. 1641 fails against a
  pre-fix build.

### `#{pane_dead_signal}` printed the number, not the signal name

- **What it printed:** `15` for a pane killed by `SIGTERM`, where the reference
  prints `term` on the same host. The default `remain-on-exit-format`
  (`options-table.c`) interpolates it, so a dead pane's own status line carried
  the wrong text.
- **Cause:** `sig2name` (`tmux.c:309`) had never been ported;
  `format_cb_pane_dead_signal` formatted `WTERMSIG(status)` directly.
- **Fix:** `sig2name` ported with the platform split the C gets from configure:
  `sys_signame` is a BSD interface, so the name table is used on Apple targets
  and the number is printed where the C's `HAVE_SYS_SIGNAME` would be undefined
  (glibc, musl). The reference behaves the same way on each platform, so parity
  holds on both while the text differs between them.
- **Pinned by:** case 1598, which kills one pane with `exit 3` and another with
  `SIGTERM` under `remain-on-exit on` and reads back
  `#{pane_dead_status}`/`#{pane_dead_signal}`, polling for the panes to die
  rather than sleeping. It fails against a pre-fix build.

### Session and window names went through a sanitiser upstream deleted

- **What it did:** `rename-session sess.dot` stored `sess_dot`, and an empty name
  was refused outright. `new-session -n` and `new-window -n` validated nothing,
  so a name with a control character in it reached the window as-is.
- **Cause:** both rename and `new-session -s` called `session_check_name`, the
  pre-3.7 sanitiser that rewrote the `.` and `:` target separators to `_` and
  rejected an empty string. next-3.7 deleted that function; it validates with
  `check_name` and escapes with `clean_name` (`tmux.c:299`, `:285`) instead, at
  five call sites: `cmd-rename-session.c:54-61`, `cmd-new-session.c:102-121`
  (window name, session name) and `:155-160` (session-group prefix), and
  `cmd-new-window.c:73-83`.
- **Fix:** `check_name` ported, a `clean_name_string` helper added for the
  callers that want an owned name, and all five sites moved onto them with the
  C's own error wording (`invalid session name:` / `invalid window name:` /
  `invalid session group name:`). The dead sanitiser was removed; the unit test
  that pinned its `.`/`:` rewriting now pins what the C does, including that a
  control character fails `check_name` before `clean_name` is reached — which is
  what the reference prints for `rename-session "a<TAB>b"`.
- **Pinned by:** cases 1708 (rename, dots/colons/empty for both objects) and
  1707 (the same names given to `new-session -s`/`-n` and `new-window -n`).

### `list-commands <name>` never failed, and could not abbreviate

- **What it did:** `list-commands nosuchcommand` printed nothing and exited 0
  where tmux prints `unknown command: nosuchcommand` and exits 1; and
  `list-commands new-w` printed nothing where tmux resolves the abbreviation and
  prints `new-window`'s usage.
- **Cause:** the port walked `CMD_TABLE` filtering on an exact name or alias
  match — the older tmux shape. next-3.7 calls `cmd_find`
  (`cmd-list-commands.c:95`), which resolves unique prefixes and hands back a
  cause to report when it cannot.
- **Fix:** ported as the C has it, `cmd_list_single_command` included, with the
  no-argument path still walking the whole table.
- **Pinned by:** case 1704, which asks for a full name, an alias, an
  abbreviation, an unknown name and an ambiguous prefix. The full listing is
  deliberately not counted: ztmux's table carries its own extension commands.

### Every failed client-side read was reported as a successful empty one

- **What it did:** `source-file` on a directory printed nothing and returned 0.
  tmux prints `Input/output error: <path>` and returns 1 — and `-q` does not
  silence it, because the C only skips a quiet `ENOENT` (`cfg.c`, load_cfg). The
  same silence covered a file with no read permission and any other read that
  fails after the open succeeds.
- **Cause:** `file_read_error_callback` (`src/ported/file.rs`) took the
  bufferevent's `what` argument, ignored it, and sent `msg_read_done` with
  `error: 0`. The C sends `msg.error = (what & EVBUFFER_ERROR) ? EIO : 0`
  (`file.c:687`), so an errored read is reported as an error and an EOF as a
  clean end. With the flag dropped, the server could not tell the two apart and
  treated every failure as "read fine, nothing in it".
- **Fix:** the `what` flag is honoured, with `EVBUFFER_ERROR` (0x20) spelled out
  where the event loop's own constant is private.
- **Pinned by:** case 1914, which sources a directory and an unreadable file,
  each with and without `-q`, and then checks the option that file would have set
  was not set.

### Thirteen usage strings had drifted from the C

- **What they showed:** `choose-tree`, `choose-client`, `choose-buffer` and
  `customize-mode` omitted flags they accept (`-k`, and `-h`/`-i`);
  `command-prompt` omitted `-F`/`-N`/`-P`; `break-pane` named its `-x`/`-y`/`-X`/
  `-Y` arguments wrongly; `display-menu` and `display-popup` had lost a space
  before `[-T title]`; `send-keys` and `send-prefix` printed `-t target-pane`
  as required rather than optional; `server-access` had no `-t` at all; and
  `bind-key`, `new-session`, `respawn-pane`, `respawn-window`, `set-buffer` and
  `show-hooks` each ended with the wrong optional argument.
- **Cause:** ~90 usage strings were transcribed by hand and are data, so neither
  the anti-drift gate (function names) nor any case had compared them.
- **Fix:** all thirteen corrected against their C entries.
- **Pinned by:** case 1791, which diffs the entire `list-commands` output and
  excludes only the six lines that must differ (ztmux's five `list-*` commands
  carry their structured-output flags; `znative` exists only here). Any future
  drift in any usage string, name or alias now goes red.

### `#{L:…}` did not set its loop variables

- **What it did:** inside the client loop, `#{loop_index}` and
  `#{loop_last_flag}` expanded to nothing, where the session, window and pane
  loops set both.
- **Cause:** `format_loop_clients` was missing the two `format_add` calls the C
  makes on the per-client tree (`format.c:5075-5076`).
- **Fix:** both added, in the C's position (before `format_defaults`).
- **Pinned by:** case 1778, with a real client attached through the
  nested-client technique.

### The client-information modifier was unimplemented

- **What it did:** `#{I/f:RGB}`, `#{I/c:Ms}` and `#{I/e:VAR}` expanded to nothing
  even with a client attached; tmux answers `1`/`0` and the variable's value.
- **Cause:** `I` was absent from the modifier tokenizer's with-arguments set,
  from the modifier parse and from the apply step, and neither helper it needs
  (`tty_term_has_name`, `tty_feature_present`) had been ported.
- **Fix:** both helpers ported from `tty-term.c:781` and `tty-features.c:604`,
  the three `FORMAT_CLIENT_*` flags added, and the apply block ported where the C
  has it (`format.c:5428-5457`) — including its early return of an empty string
  when there is no client or the client is unattached.
- **Pinned by:** cases 1777 (no client: empty, no error) and 1776 (a real client:
  a feature, a capability and an environment variable it sets).

### `alert-activity` fired again on every alert pass

- **What it did:** a window that stayed flagged kept notifying: a run that should
  fire `alert-activity` twice and `alert-bell` once fired activity three times
  and then again after the bell. Deterministic, both runs identical.
- **Cause:** `alerts_check_activity` was missing the C's
  `if (wl->flags & WINLINK_ACTIVITY) continue;` (`alerts.c:151`), which stops a
  winlink that is already flagged from notifying again. The bell check has no
  such guard by design (the C says so in a comment), so only the activity path
  was wrong.
- **Fix:** the guard added, with the C line cited and the bell asymmetry noted.
- **Pinned by:** case 1784, which drives activity and a bell from pane output and
  compares the sequence of hooks that fired.

### `send-prefix -2` killed the server

- **What happened:** `send-prefix -2` on a session with the default `prefix2`
  (`None`) took the whole server down — every other client on it died with
  `server exited unexpectedly`. tmux sends the key and returns 0.
- **Cause, in two halves.** `prefix2` defaults to `KEYC_NONE`, so the "no key"
  sentinel is what reaches `input_key`. (a) `KEYC_IS_UNICODE` said yes to it:
  the C tests the key's TYPE field against `KEYC_TYPE_UNICODE` (`tmux.h:201`),
  which this port cannot do while it carries the flat `keyc` encoding, and the
  older "above 0x7f and not a special key" test it uses instead does not exclude
  `KEYC_NONE`/`KEYC_UNKNOWN`, which live inside `KEYC_MASK_KEY` here. (b) That
  sent the sentinel into `utf8_to_data` → `utf8_get_width`, which computes
  `(uc >> 29) - 1`: an unsigned wrap in C (`utf8.c:257`) and a debug-build
  `attempt to subtract with overflow` panic here.
- **Fix:** the two sentinels are excluded from `KEYC_IS_UNICODE` with a comment
  naming the encoding gap that makes the exclusion necessary, and
  `utf8_get_width` uses `wrapping_sub` to keep the C's arithmetic. Either fix
  alone stops the crash; both are wrong without the other.
- **How it was found:** the crash dump `~/.ztmux/server-panic-<pid>.txt` that
  the server's panic hook writes named `utf8_get_width` → `utf8_to_data` →
  `input_key` → `window_pane_key` → `cmd_send_keys_inject_key` directly.
- **Pinned by:** case 1689, which sends the prefix and then `-2` with `prefix2`
  both unset and set, and asserts the server is still answering afterwards.

### The file-error message had its two halves the wrong way round

- **What it printed:** `/tmp/nowhere/out.txt: No such file or directory` for a
  failed `save-buffer`, where tmux prints
  `No such file or directory: /tmp/nowhere/out.txt`. Same for `load-buffer`.
- **Cause:** both done-callbacks formatted `"{path}: {strerror}"`; the C formats
  `"%s: %s", strerror(error), path` (`cmd-save-buffer.c:68`,
  `cmd-load-buffer.c:69`).
- **Fix:** the two arguments swapped at both call sites, matching the C.
- **Pinned by:** case 1653, which asks for a save into a directory that does not
  exist, a load of a file that does not exist, and a save from a buffer name that
  does not exist (the last one was already right). It fails against a pre-fix
  build.

## 2026-08-21 (the queued request/reply mechanism)

### Panes could not ask the terminal anything, and four entries hung off that

The C lets a pane ask the terminal a question it cannot answer itself -- what a
palette entry is (OSC 4), what the clipboard holds (OSC 52) -- by forwarding the
question to a client and routing the answer back when it arrives. None of that
existed here: fifteen `input_*` functions, the per-pane and per-client queues
they walk, and the ordering rule that makes an asynchronous answer safe.

- **What a pane saw before:** an OSC 4 query for an entry the pane had no local
  value for got silence; an OSC 52 query got the newest paste buffer regardless
  of what `get-clipboard` said; and `DCS $ q ... ST` (DECRQSS) got nothing at
  all, because `input_handle_decrqss` had no counterpart. Silence is the worst
  of these: the program inside the pane waits for a reply that never comes.
- **Ported, where the C has them:** `input_make_request`, `input_add_request`,
  `input_free_request`, `input_cancel_requests`, `input_request_reply`,
  `input_request_palette_reply`, `input_request_clipboard_reply`,
  `input_request_timer_callback`, `input_start_request_timer`,
  `input_send_reply`, `input_handle_decrqss`, `input_osc_52_parse`,
  `input_osc_52_reply`, `input_start_ground_timer` and
  `input_ground_timer_callback` (the last two existed but were inlined and
  named `timer`, which is why the C's `ground_timer` name is back).
- **Ordering is the point, not a detail.** `input_reply` gains the C's `add`
  argument (`input.c:1153`): with it set, a reply that would overtake an
  outstanding request is queued behind it instead of being written now, so a
  pane sees its answers in the order it asked. Seventeen of the port's
  twenty-one reply sites pass `add=1`, exactly as the C does. A request that is
  never answered is dropped after 500ms by the request timer, which flushes
  whatever queued behind it -- otherwise one silent terminal would stall a pane
  permanently.
- **Two signatures were still in their pre-next-3.7 form** and are corrected
  with it: `input_osc_colour_reply` gains `add`, `idx` and the terminator
  (`input.c:2873`) -- and with `idx` the OSC 4 reply finally names the entry it
  is answering about -- and `input_reply_clipboard` gains `clip`
  (`input.c:3336`), so a reply names the selection it came from.
- **A palette lookup was using the wrong key.** `input_osc_4` looked the entry
  up as a bare index where the C uses `idx|COLOUR_FLAG_256` (`input.c:2924`), so
  a query could miss an entry that was set.
- **`refresh-client -l` is fixed by the same change**, which is why it was
  blocked on it. The port declared `l::` where the C declares `l`
  (`cmd-refresh-client.c:39`), so `-l` swallowed the next character as its
  value and `refresh-client -lZ` was accepted as "-l with value Z" where the C
  rejects an unknown flag. Underneath it still implemented the pre-next-3.7
  `-l [target-pane]` semantics with `clipboard_panes` and a
  `CLIENT_CLIPBOARDBUFFER` flag that exists nowhere in `vendor/tmux`; next-3.7
  is just `tty_clipboard_query`, and the queue routes the answer to whichever
  pane asked. The two `clipboard_panes` FIELDS stay, because the C still
  declares (`tmux.h:2311`) and frees (`server-client.c:475`) them while reading
  them nowhere; the flag is gone, because the C does not have it.
- **`TTY_WAITBG`/`TTY_WAITFG` are fixed by it too**, for the same reason: their
  only consumer is the key-delay condition at `tty-keys.c:979-985`, which also
  reads the request queue. Both flags are now set with the OSC 10/11 queries
  (`tty.c:409`), cleared when the start timer gives up (`tty.c:322`), and read
  where the C reads them -- a key read waits 500ms while a query is outstanding
  so a terminal's reply is never chopped up as keystrokes.
  `tty_repeat_requests` regains its `force` argument (`tty.c:414`), and with it
  the forced repeat after a theme change (`server-client.c:3110`) that the port
  was not doing at all; `tty_start_start_timer` comes back out of
  `tty_start_tty`, because `tty_repeat_requests` is its other caller.
- **The DCS dispatch is now the C's.** It no longer returns early when there is
  no pane (a popup has none and still parses DCS), it reads
  `allow-passthrough` from global window options in that case
  (`input.c:2611-2614`), and `$`-intermediate sequences route to DECRQSS.
- **Verified live, byte-for-byte against the reference**, by running a pane that
  echoes whatever the server writes to it: `DCS $ q SP q ST` returns
  `DCS 1 $ r SP q 0 SP q ST` and `DCS $ q m ST` returns `DCS 0 $ r ST` on both
  binaries. Primary DA is deliberately not compared: its reply depends on
  whether the binary was built with sixel support (`input.c:1562-1566`).
- **Pinned by:** `parity/cases/1563_decrqss_reply.sh` (both DECRQSS answers) and
  `parity/cases/1564_refresh_client_l_arity.sh` (the flag arity, including that
  `-l` no longer eats a following argument).

### Read-only clients could detach everybody else

- **Root cause:** three `CLIENT_READONLY` gates were missing. `attach-session
  -r` and `switch-client -r` let an already-read-only client re-assert or clear
  the flag with no check, where the C requires the caller to be the server's own
  user (`proc_get_peer_uid` vs `getuid`, `cmd-attach-session.c:111-117`,
  `cmd-switch-client.c:83-89`) -- clearing it is a privilege escalation, and the
  C tests the flag on the TARGET client but the uid of the CALLING one.
  `detach-client` carried `CMD_READONLY` (so a read-only client may detach
  itself) without the gate that stops it detaching anyone else
  (`cmd-detach-client.c:73-78`).
- **Coverage now:** the C has 24 `CLIENT_READONLY` sites across 9 files, the
  port 22 across the same 9. The difference is `server-client.c`, where the C
  writes the same read-only check twice -- once for `default-client-command`
  and once for a command message -- and the port reaches both through one
  shared path.
- **Pinned by:** `parity/cases/1565_readonly_client_gates.sh`, which makes a
  REAL read-only client (an inner server attached with `attach -r` from a pane
  of the outer one) and has that client press the key, because running the
  command from a fresh client would test nothing. It shows the writable client
  surviving `detach-client -a` and the read-only client still able to detach
  itself.
- **What is still untestable here:** the two uid checks need a second real
  account to exercise, so they are ported to the C's shape but not covered by a
  case.

## 2026-08-21 (structured output under a non-UTF-8 client)

### `-o json` collapsed to one unparseable line, and the extensions that read it went with it

- **Blast radius:** 102 of the modules under `src/extensions/` resolve the
  running server by parsing `-o json` (`grep -rl "\-o json" src/extensions`),
  so for a client in this state they read a document that cannot be parsed at
  all. The earlier record of this entry counted "64 of 113 verbs" flipping; that
  number was measured on 2026-08-18 and is left there rather than restated here,
  since `ztmux verbs` now reports 114 and the count was never re-run.
- **Symptom:** `LC_ALL=C ztmux list-windows -o json` emitted
  `[_{"session":...}_]` — every newline in the document replaced by `_`, so
  nothing downstream could parse it. All six formats collapsed the same way.
- **Root cause:** `Rows::render` built the whole document as one string and
  handed it to a single `cmdq_print`. `server_client_print`
  (`vendor/tmux/server-client.c:3040`, ported faithfully) runs `utf8_sanitize`
  over a message when the client lacks `CLIENT_UTF8`, and `utf8_sanitize`
  (`vendor/tmux/utf8.c:784`) replaces every byte outside `0x20..=0x7e` with `_`
  — newlines included. tmux's own listings never hit this because they call
  `cmdq_print` once per line.
- **Two conditions, and the missing one is why this looked unreproducible at
  first:** the client needs a non-UTF-8 locale AND no `$TMUX` in its
  environment. `tmux.c:485-492` assumes UTF-8 when `$TMUX` is set, so a probe
  run from inside a ztmux pane — which is where the repro was first attempted —
  gets `CLIENT_UTF8` no matter what `LC_ALL` says and prints clean output. From
  a bare shell it reproduces every time. The earlier record of this entry gave
  the `LC_ALL=C` half only.
- **Fix:** `Rows::print` prints one `cmdq_print` per line, and the five call
  sites (`cmd_list_buffers/clients/panes/sessions/windows.rs`) call it instead
  of rendering into one message. Byte-for-byte identical for a UTF-8 client:
  `cmdq_print` appends a newline per message, so splitting on `\n` reproduces
  exactly the separators the document already had.
- **What is deliberately NOT fixed:** non-ASCII *content* is still sanitized for
  such a client — `"name":"h_llo"` for a window named `héllo` — because that is
  the client declaring it cannot render UTF-8, and every other tmux listing gets
  the same treatment. A "fix" that smuggled raw UTF-8 past `utf8_sanitize` would
  be a divergence, so a test pins the sanitizing.
- **`-o tsv` remains unusable for that client, and this is not a defect:** a tab
  is `0x09`, outside the printable range, so the separators arrive as
  underscores. No line-splitting can change that — it is the C's own rule about
  what may be sent to a client that has not declared UTF-8. The man page now
  says so under `-o tsv` and points at `-o csv`, whose commas are printable.
  json, jsonl, csv, yaml and table are all intact.
- **Pinned by:** `tests/structured_output_under_c_locale.rs` — three tests, of
  which the two structural ones fail against a rebuild without the fix and the
  sanitizing one passes both ways by design.

## 2026-08-20 (n-ary boolean operators)

### `||` and `&&` were binary, so the mouse wheel stopped opening copy mode

- **Symptom:** wheel-up in an ordinary pane did nothing — copy mode never
  opened and the scrollback never moved.
- **Root cause:** `format_replace` grouped `||` and `&&` with the comparison
  modifiers and evaluated them through `format_choose(es, copy, &left, &right,
  1)`, which splits at the FIRST top-level comma. The C has them as n-ary
  operators — `format_bool_op_n` (`format.c:4686`) walks every comma-separated
  operand and short-circuits. So `#{||:0,0,0}` split into `0` and `0,0`, and
  `format_true("0,0")` is true because the string is neither empty nor `"0"`, so
  the whole expression came out `1`. Measured before the fix:
  `display-message -p '#{||:0,0,0}'` printed `1` under the port and `0` under
  both `vendor/tmux` (next-3.7) and the system tmux 3.7b.
- **Why that reached the wheel:** the default binding is
  `bind -n WheelUpPane { if -F '#{||:#{alternate_on},#{pane_in_mode},#{mouse_any_flag}}' { send -M } { copy-mode -e } }`
  (`key-bindings.c:457`). In a plain shell pane all three operands are `0`, so
  the condition is false and the else branch opens copy mode. Read as two
  operands it was unconditionally true, so every wheel event took `send -M` and
  was forwarded to a pane that had never asked for the mouse — which drops it.
- **When it broke:** `31ec4f7cf8` (2026-08-10) brought that binding to
  upstream's three-operand form. The port's binding had been the older
  two-operand text, which the binary reading happened to evaluate correctly, so
  the defect was latent until the binding itself became faithful.
- **Fix:** `format_bool_op_n` ported as the C has it, `||` and `&&` moved out of
  the `cmp` group into `bool_op_n` (`format.c:5414-5417`), and dispatched
  between the `!!` branch and the comparison branch where the C dispatches them
  (`format.c:5572-5577`). The `||`/`&&` arms in the comparison chain are gone;
  the C has none. Short-circuiting comes with it — `&&` stops at the first false
  operand, `||` at the first true one, so `#{&&:0,#{e|/|:1,0}}` never expands the
  division, matching the reference.
- **Why the suite missed it:** all sixteen existing `||` cases used exactly two
  operands, where the binary reading and the n-ary one agree.
- **Blast radius beyond the wheel:** every other `||`/`&&` in the default
  bindings and options table is two-operand, so `WheelUpPane` was the only
  default site affected. Any user format with three or more operands was wrong
  the same way.
- **Pinned by:** parity cases 1557-1561 (the operator itself, including nested
  operands whose commas must not split the list) and 1562 (the wheel condition,
  in and out of copy mode), plus the unit test
  `test_format_expand_boolean_operators_are_n_ary`. Cases 1557, 1560, 1561 and
  1562 were checked against a rebuild WITHOUT the fix and fail there; 1558 and
  1559 pin the true path, which both readings get right.

## 2026-08-18 (tty_style_ctx, and the last style directive)

### `dim=` was rejected, because the port had nowhere to put it

`dim=` was the one directive of the fifty-odd that the two binaries still
disagreed on. It was recorded open rather than half-ported, because accepting it
is trivial and honouring it is not: the C carries the percentage in
`struct tty_style_ctx` (`tmux.h:1686`) and `tty_attributes` dims the resolved fg
and bg through `colour_dim` (`tty.c:2649-2659`). This port passed `defaults`,
`palette` and `hyperlinks` as three separate parameters, so `dim` had nowhere to
live. A parse-only version would have stored a value nothing read — a config that
renders undimmed while looking applied.

So the struct came over, as upstream has it. `tty_cell`, `tty_attributes`,
`tty_default_attributes` and `tty_draw_line` take one `*const tty_style_ctx`
instead of three parameters; `tty_ctx` carries it where it carried a bare
palette; callers with no pane context pass NULL exactly where the C does.
`tty_default_colours` regained its `u_int *dim` out-parameter and
`tty_style_changed` split back out of it, because that is where the dims are
produced — `style_add` now returns the resolved style (`style.c:462`) and
`sy->dim` is cached beside each cell, so `window-style` and `window-active-style`
dim independently.

**Two things had to land inside `tty_attributes` for the dim to mean anything.**
`colour_dim` returns a THEME colour untouched and a DEFAULT colour untouched —
neither has RGB to scale — so the C resolves theme colours and substitutes a
concrete colour for the default (`tty_dim_default_colour`, `tty.c:2598`) BEFORE
dimming. This port did its theme mapping down in `tty_check_fg`/`_bg`/`_us`
instead, which is too late: by then the value has already been compared against
`last_cell` and handed to `tty_colours`. The C's block at `tty.c:2637-2659` is
now ported where the C has it, and `tty_check_fg` does the palette lookup and
theme mapping a second time as it does upstream — both are idempotent.

Verified through a nested client, byte-for-byte: colour196 on colour21 at
`dim=50` emits `38;2;127;0;0` / `48;2;0;0;127` (the 256-palette form is forced to
RGB by the dim), `dim=100` collapses to black on black, `dim=25` on red/blue
gives `96;0;0` / `0;0;96`, and `dim=0` draws exactly what no dim draws. Case 1556
pins that plus the accept/reject set, and deliberately pins the UNDIMMED
default-colour path too: under the suite the inner terminal reports no fg/bg and
has no theme, so `tty_dim_default_colour` returns the colour unchanged and no
colour sequence is emitted at all. That is the branch a future "just dim it
anyway" would break.

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
