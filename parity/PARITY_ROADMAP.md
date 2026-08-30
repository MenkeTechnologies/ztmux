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

## Quarantine

`parity/quarantine.txt` lists cases whose result is reported but does not gate. A case goes
there only when it fails somewhere we cannot reproduce, so leaving it in the gate would keep
CI red with nobody able to work on it.

It is not a way to retire a case. Every quarantined case is still executed and still diffed
on every run; a divergence prints as `QUARANTINED-FAIL`, its full diff lands in
`parity_failures.log`, and the count rides next to the percentage on the summary line and in
the JSON (`quarantined`, `quarantined_failing`, with `passed + failed + quarantined ==
total`). The percentage is of the gated set, so the headline can never read better than the
tree is. When a quarantined case starts matching, the runner says `QUARANTINED-OK` and names
the file to delete the line from.

```sh
bash parity/run_parity.sh                            # honours parity/quarantine.txt
bash parity/run_parity.sh --quarantine ''            # gate everything, nothing excused
```

**Open: fixed-sleep fences flake under suite load (2026-08-24).** Several of the
attached-client render cases wait with a bare `sleep 2` / `sleep 0.9` instead of polling for
a marker the way 1531 and 1547 do. Under a full run, where the next case's servers are
already starting, that races: 1526 came back with EMPTY output in one macOS run (the case's
`timeout 15` expired) and passes on every isolated re-run, and 1528 fails about one run in
two. They are not divergences and are not quarantined; the fix is to fence them on a marker.

**Open: twelve render cases, Linux only (2026-08-24).** next-3.7 styles the status line and
menus with theme colours (`status-style` defaults to `bg=themegreen,fg=themeblack`), which
`server_client_update_theme_colours` resolves per client from the `dark-theme-*` options into
RGB. ztmux does that on macOS — the twelve pass locally against `vendor/tmux/tmux` with both
the debug and the release binary, and the full suite is green — but on `ubuntu-latest` every
one of them diffs the same way: the reference emits RGB and ztmux emits the terminal ANSI
values (`^[[37m`/`^[[40m`/`^[[42m`, and nothing where the entry resolves to 8), which are
`colour_theme_terminal_colour`'s. Both sides negotiate the same feature set including `RGB`
(case 1555 prints it). One bug, twelve symptoms; it needs a Linux reproduction to find.

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

**1633/1633 cases pass (100%) vs the vendored tmux — one known divergence, recorded as a gap.** The
suite grew from 122 → 380 → 646 → 661 → 665 → 675 → 680 → 684 → 686 → 689 → 774 → 840 → 900 → 1080 → 1107 → 1115 → 1121 → 1123 → 1130 → 1134 → 1166 → 1173 → 1178 → 1180 → 1183 → 1188 → 1193 → 1194 → 1201 → 1203 → 1205 → 1207 → 1240 → 1244 → 1245 → 1251 → 1254 → 1339 → 1365 → 1389 → 1405 → 1417 → 1426 → 1433 → 1446 → 1452 → 1480 → 1495 → 1525 → 1598 → 1613 → 1618 → 1630 → 1633 cases.

**Cases 1926–1945 came from a flag audit.** Every `.args` string in
`vendor/tmux/cmd-*.c` was diffed against the whole corpus, which named 107 flag
letters no case had ever passed. Writing cases for the first of them turned up
four port defects rather than confirming the surface: `split-window`'s exec had
drifted from next-3.7 wholesale (`-E` did not make an empty pane, the
`command cannot be given for empty pane` refusal was absent, and the whole
post-spawn block that puts `-s`/`-S`/`-R`/`-B`/`-k`/`-m`/`-T` on the new pane was
missing), `join-pane -p` read the value of flag `l` and so failed with
`size missing` for every percentage, and `split-window -k` took the server down
because `remain-on-exit` was missing next-3.7's fourth choice `"key"` — the C
writes 3 into an option this tree only had three names for. Porting
`layout_get_tiled_cell` (`layout.c:1593`) for the first two also **closed the
`join_pane_before_placement` gap**, which had recorded exactly the missing
wrapper; its case is now 1943. `docs/BUGS.md` carries the write-ups.

**Two harness traps, both of which produced false results before they were
caught.** `verify_one.sh` takes a *path*: given a bare case name it ran
`bash NAME.sh` for both binaries, both failed identically with `No such file`,
the byte comparison matched, and it printed `OK`. Every bare-name check was a
pass no matter what the case said; it now refuses to run on a case file it
cannot read. And `run_parity.sh` builds `target/release/ztmux` only when that
binary is **absent** — it never rebuilds a stale one, so a suite run started
after a source change measures the previous build. Two runs this round reported
failures that were only that. Rebuild the release binary yourself before
trusting a run that follows a code change.

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
away by the round trip. **278 of the 283 bindings are compared**; the 19 keys still
excluded are listed by key in the case with the reason for each, and the list
shrinks as the features behind them land — the 32 `command-prompt -P` keys left
it when the in-pane prompt landed, and the three pane-menu keys left it when the
truncated comparison replaced them.

A `SKIP` entry used to make a key **invisible rather than merely uncompared**:
the case passed with `prefix >` deleted outright and `root MouseDown3Pane` rebound
to something unrelated, and the `compared N` guard could not catch it because `N`
counts only non-skipped lines. The three pane-menu keys — which carry roughly
3.3 KB of C-derived menu content that no other case covers — are now compared up
to the last row the C defines (`z { resize-pane -Z }`, `key-bindings.c:72`), with
`NO-MARKER` printed if it is absent, so deletion and gutting both go red. The
remaining 19 are structurally uncomparable by a parity case: 14 exist only in
ztmux and 5 only in the reference. The output is sorted, because `list-keys` walks each
table in key-code order and ztmux's flat `keyc` enum orders differently from the
C's type-shifted one — so the case compares the set of bindings and their
commands, not their order, until that encoding migrates. Both halves were
mutation-tested: reintroducing the `MouseDown1Status` command and dropping one
`--` each turn the case red.
Cases **1499–1509** are an acceptance round rather than a probe block: they take
[hashrocket/dotmatrix](https://github.com/hashrocket/dotmatrix)'s `.tmux.conf` —
a config a whole shop runs — and ask whether ztmux loads and executes it the way
tmux does. The config is written out and read with `source-file` rather than
replayed as `$TM set …` lines, so the config LEXER runs (`\;` chains, `-q`
mid-arguments, comments, quoting) and not just the command parser. 1499 covers its
options (a window option set globally, `set -sa` appending to a server option that
already holds a default, `-sg` on a server option, `-q`), 1500 its bindings read
back one key at a time, 1501 the four copy-mode commands it binds driven end to
end, 1502 `send-keys -R` against a pane left in five sticky terminal states, 1503
the conditional `if-shell` include it ends with. Every option and every binding the
config sets was already identical; the round's two bugs were underneath it, and
both are pinned here:

- **1504** builds a terminal the harness does not have — a second server inside a
  pane of the first, with a client attached to it — so `capture-pane -e` on the
  outer server re-serialises what the inner client actually drew. That is what
  caught `tty_map_theme_colour` being unported: next-3.7's default `status-style`
  is `bg=themegreen,fg=themeblack`, and a theme colour stores and prints back
  perfectly while never resolving at render time, so the whole status bar drew
  unstyled while every `show-options` case stayed green.
- **1505** runs eleven target shapes through `attach-session`. The C picks the
  target type with `tflag[strcspn(tflag, ":.")] != '\0'` — "contains `:` or `.`";
  the port asked whether anything remained after stripping *leading* separators,
  which is true for every ordinary name, so a window name resolved as a pane
  target and `attach -t <window>` was accepted where tmux refuses.

Both were mutation-tested: reverting either fix turns its case red.

Cases **1506–1507** close the in-pane prompt gap. next-3.7 moved the prompt off
the status line and into the pane for the copy-mode bindings that take input, and
32 of the default bindings carry the `-P` that asks for it. The port had the flag
in its `prompt_flags` enum and nothing behind it, so the default table had been
written without `-P` to match what the port could do — which made the gap
self-consistent and invisible. Closing it needed `window_pane` to gain
`prompt`/`prompt_data`/`prompt_cx`, `window.c`'s five pane-prompt functions and
two callbacks, the `-P` dispatch in `cmd-command-prompt` (including the
multi-prompt update branch that writes the next question back to the pane), the
key routing that prefers the active pane and falls back to the first visible pane
holding a prompt, and `redraw_draw_pane_prompt`. 1506 pins the flag and the 32
bindings; 1507 pins where the prompt lands, through a client, asserting the status
row as well as the prompt row — a port that drew the prompt correctly but ate the
status bar would otherwise pass. The 32 keys left case 1498's exclusion list at the
same time, so they are blocking again.

Cases **1508–1509** close the last two client-visible gaps this round found.
1508 compares all four mode-tree screens — `choose-tree`, `choose-client`,
`choose-buffer` and `customize-mode`, i.e. `prefix w`/`s`/`=`/`D` — through an
attached client. None of that was reachable server-side, because `mode_tree_draw`
only runs for a client, so the whole row composition had drifted to an older
revision unnoticed: no `MODE_TREE_PREFIX_FORMAT`, no per-depth alignment, a
hand-composed row string, and four mode format-string constants each frozen at an
older tmux (`#[reverse]` where the C has `#[fg=thememagenta]`, missing
`#[fg=themelightgrey]`, and the C's multi-branch `#{?a,b,c,d,e}` rewritten as
nested conditionals). The case asserts the box title as well as the rows, which is
what caught a missing `mode_tree_view_name` call.

1509 pins non-UTF-8 byte handling. The port dropped invalid bytes where the C
enters U+FFFD, so every column after one shifted. The case asserts **both**
directions — invalid bytes producing replacement characters *and* valid CJK,
emoji and box-drawing rendering untouched — because the failure mode of getting
`utf8started`'s ordering backwards is worse than the original bug.

Both were authored against the nested-client technique cases 1504 and 1507
introduced, which is now the only way this suite can see anything a client draws.

Cases **1566–1650** were chosen by measuring what the suite did *not* touch
rather than by deepening what it already did. Two inventories drove it: the
command list (`wait-for`, the `%if`/`%elif`/`%else`/`%endif`/`%hidden` config
conditions, `source-file`'s `-q`/`-n`/`-v`, `show-options -A`/`-v`/`-q` and its
scope flags, command-name resolution and its errors, `bind`/`unbind`/`list-keys`
flags and key tables, `command-alias`, `run-shell`, `pipe-pane`) and the format
table, diffed against every case file: 94 of the 195 format variables had never
appeared in a case. The block covers 48 of them — the name-exists modifier
`#{N/w:}`/`#{N/s:}`, window and session ids and stacks, session groups, the
marked-session flag, linked and active session counts and lists, the dead-pane
status and signal, cell geometry, the mouse variables outside a mouse key, and
`#{config_files}`. It found two divergences, both now fixed and both pinned:

- **`#{window_linked_sessions}` counted winlinks, not sessions** (1641) —
  `format_cb_window_linked_sessions` returned `window->references`, which is the
  older tmux implementation. The C (`format.c:2919`) counts one per session group
  holding the window plus each ungrouped session holding it, so a window linked
  twice into the *same* session counts once. The case pins exactly that shape: a
  window in two sessions and three winlinks reads `n=2` with a three-entry
  `#{window_linked_sessions_list}`.
- **`#{pane_dead_signal}` printed the signal number where tmux prints its name**
  (1598) — `sig2name` (`tmux.c:309`) had never been ported. It returns
  `sys_signame[signo]` when configure found that table (`HAVE_SYS_SIGNAME`) and
  the number otherwise, which is a platform split, not a preference: `sys_signame`
  is a BSD interface, and glibc and musl do not have it. The port expresses the
  same split as a target gate on Apple targets, so a pane killed with `SIGTERM`
  reads `term` on macOS and `15` on Linux — matching the reference on each, which
  is what the case compares.

Both cases were run against the pre-fix binary and fail there.

Cases **1651–1676** continue the same sweep through the command list: the buffer
file commands (`save-buffer` incl. `-a` and `-`, `load-buffer`, `paste-buffer`
with `-s`/`-d`), `send-keys` (`-l`, `-H`, `-N`, and key-name lookup),
`select-pane -T` and `#{pane_title}`, `new-session` `-A`/`-P`/`-F`/`-e`,
`set-option -F` and `-p`, `source-file -F`, `kill-session -a`/`-C`,
`list-panes -s`/`-a`/`-f`, `set-hook -R` and hook arrays, `capture-pane`
`-J`/`-N`/`-e`/`-C`/`-b`, `if-shell -F`, `select-layout -E`/`-o`,
`resize-pane -Z`, `move-pane -b`, `respawn-window -k`, and the "no current
client" path of the client-only commands. One divergence, now fixed:

- **the file-error message had its two halves the wrong way round** (1653) —
  `cmd_save_buffer_done` and `cmd_load_buffer_done` both formatted
  `"{path}: {strerror}"` where the C formats `"%s: %s", strerror(error), path`
  (`cmd-save-buffer.c:68`, `cmd-load-buffer.c:69`). So a failed save read
  `/tmp/x: No such file or directory` instead of tmux's
  `No such file or directory: /tmp/x`. Both call sites corrected; the case
  fails against a pre-fix build.

Cases **1677–1700** take the third pass: the target-token syntax nothing had
touched (`{start}`/`{end}`/`{last}`/`{next}`/`{previous}` and `+N`/`-N` for
windows; `{top}`/`{bottom}`/`{left}`/`{right}`, the four corners and the
`{up-of}` family for panes; `~`/`{marked}`, `=`/`{mouse}`, `$`/`@`/`%` ids, the
`session:window.pane` string form and the `=name` exact-match prefix), option
arrays (`set -a` with a subscript, `-o`, `-q`, `terminal-features[]`), the
window and pane creation flags (`new-window -k`/`-S`/`-a`/`-b`/`-P -F`,
`split-window -l`/`-p`/`-b`/`-f`/`-Z`, `break-pane`, `swap-pane -D`/`-U`/`-d`,
`kill-pane -a`), `run-shell -C`, `pipe-pane -I`/`-O`, `list-windows -f`,
`display-message -a`, `send-prefix`, and the client-only commands' error paths.
One divergence, and it took the server down:

- **`send-prefix -2` killed the server** (1689) — `prefix2` defaults to
  `KEYC_NONE`, so the key that reaches `input_key` is the "no key" sentinel. Two
  things then went wrong. `KEYC_IS_UNICODE` answered *true* for it: the C asks
  whether the key's TYPE field is `KEYC_TYPE_UNICODE` (`tmux.h:201`), which this
  port cannot ask because it still carries the flat `keyc` encoding, and the
  older "is it above 0x7f and not a special key" test it uses instead swallows
  both sentinels. That sent `KEYC_NONE` into `utf8_to_data`, where
  `utf8_get_width` computed `(uc >> 29) - 1` on a zero width — an unsigned wrap
  in C (`utf8.c:257`), a debug-build overflow panic here. Both were fixed:
  `KEYC_NONE`/`KEYC_UNKNOWN` are excluded from `KEYC_IS_UNICODE`, and the width
  macro wraps like the C's. tmux sends the key harmlessly; so does the port now.

While writing the target-token cases the reference itself was found to crash:
`display-message -p -t '{active}'` (and `{current}`) takes the vendored next-3.7
server down when no client is attached. Those two tokens are therefore absent
from case 1696, which says so — there is no reference behaviour to compare
against. Every other token in that family (`~`, `{marked}`, `=`, `{mouse}`)
expands to nothing on both binaries and is compared.

Cases **1701–1716** are the fourth pass, over the session- and window-level
commands and the options behind them: `link-window -k`, `swap-window -d`,
`move-window -r` (with `base-index`), `kill-window -a`, `set-environment`
`-h`/`-r`/`-u`, `switch-client` and `attach-session` without a client,
`run-shell -c`, `list-commands <name>`, `destroy-unattached`, `exit-empty` (a
server with no sessions at all), `fill-character`/`scroll-format`, the numeric
option bounds, `lock-command`, and the name validation. Two divergences, both
fixed:

- **`list-commands <name>` never failed and could not abbreviate** (1704) — the
  port filtered the command table by exact name or alias, which is the older
  tmux shape. next-3.7 looks the name up with `cmd_find`
  (`cmd-list-commands.c:95`), so `new-w` resolves like it does on a command line
  and an unknown name reports `unknown command: <name>` with a non-zero status.
  The port printed nothing and exited 0. Ported as the C has it, including
  `cmd_list_single_command` as its own function.
- **session and window names were sanitised by a function upstream deleted**
  (1707, 1708) — `rename-session` and `new-session -s` ran the pre-3.7
  `session_check_name`, which rewrote the `.` and `:` target separators to `_`
  and refused an empty name; `new-session -n` and `new-window -n` validated
  nothing at all. next-3.7 checks with `check_name` and escapes with
  `clean_name` (`tmux.c:285`, `:299`) at all four sites plus the session-group
  prefix, so `rename-session sess.dot` keeps its dot, an empty name is accepted
  for a window, and a name holding a control character is refused with
  `invalid session name:` / `invalid window name:`. All five call sites ported;
  the dead sanitiser is gone and the unit test that pinned its behaviour now
  pins the C's.

Cases **1717–1728** are the fifth pass and the first to find nothing: the hooks
(after-<command> hooks firing and being unset again, `window-linked` /
`window-unlinked` / `session-created` / `session-closed`, `pane-died` versus
`pane-exited` either side of `remain-on-exit`, a window-scoped hook against the
global one, and the `#{hook}` / `hook_*` formats inside a hook body — which the
notification hooks fill in and the after-<command> hooks leave empty), the
option scope chain (a user option set at all four scopes, `show-options -A` on a
window and a pane), `update-environment` as an array, the session-group and
window-client formats, `#{pane_key_mode}`, and the pane path and history-byte
formats. All twelve matched the reference first time.

Cases **1729–1737** are the sixth pass, over the flags that change an object's
shape rather than its identity: `rotate-window -D`/`-U` (followed by pane id, so
the direction is visible) and `-Z`, `resize-window -x`/`-y`/`-D`/`-R`/`-A` under
`window-size manual`, the deprecated `select-pane -P`/`-g` styles read back
through `#{pane_fg}`/`#{pane_bg}`, `paste-buffer -r`, `load-buffer -w`,
`break-pane -W` with its geometry flags, the `allow-rename` /`allow-set-title` /
`allow-passthrough` window options (including the rename escape sequence
actually being gated), and the `t` format modifier in its `/f`, `/p`, `/r` and
bare forms. All nine matched.

`split-window -W` has no case: on the vendored reference that command prints its
`-P` line and then never returns (the server keeps serving other clients; that
one client hangs), so there is no stable behaviour to compare. It is the second
upstream hang this round found, after `display-message -p -t '{active}'`.

Cases **1738–1744** close the last gap in the copy-mode command table. Of its 95
entries, 18 had never appeared in a case: the `*-and-cancel` variants
(`copy-line`, `copy-end-of-line`, `copy-selection`, `append-selection`,
`cursor-down`, `page-down`, `halfpage-down`, `scroll-down`), the `copy-pipe`
family (`-line`, `-end-of-line`, `-no-clear` and their cancelling forms),
`pipe-no-clear`, and the three search commands
(`search-forward-incremental`, `search-backward-incremental`,
`search-backward-text`). Each case asserts both halves of what the command does
— the buffer or cursor moved AND whether the mode was left — because a command
that cancels when it should not, or copies the wrong extent, otherwise looks the
same from one side. Two behaviours worth naming, both matched: `cursor-down-and-cancel`
stays in the mode while there is a line left to move onto, and the two
incremental searches leave the cursor alone when they are driven from a command
line, since their state belongs to the interactive prompt.

`scroll-to-mouse` needed a case of its own (1744): with no mouse event behind it
that command takes the server down, on the reference as well as here. The port
reproduces the upstream defect exactly, so the case pins the crash rather than
excusing it — if either side ever stops crashing, or crashes differently, it goes
red. With that, all 95 entries of the copy-mode command table are exercised by at
least one case.

Cases **1745–1752** do to the options table what 1738–1744 did to the copy-mode
one. Of its 180 entries, 42 had never been named by a case: the twenty
`dark-theme-*` / `light-theme-*` colours, the old `status-bg` / `status-fg` pair
and the per-side and per-state status styles, `message-line`, `extended-keys`
and `extended-keys-format` (and `xterm-keys`, which they replaced),
`assume-paste-time`, `prefix-timeout`, `prompt-history-limit`, `default-size`,
`default-command`, `key-table`, the four prompt-cursor options, `pane-colours`
and `user-keys` as arrays, `scroll-on-clear`, `visual-silence`,
`detach-on-destroy`, `exit-unattached` and `remain-on-exit-format`. Each is
checked for its default, the values its type accepts, and its refusal of one
that it does not — the last part being where a hand-written option table drifts
first. Two of them end the server on purpose and say so: `exit-unattached on`
with nothing attached, and (in 1712) `destroy-unattached on`.

Cases **1753–1757** finish the format table off the same way. After 1566–1650
there were 36 variables left with no case; 1753–1756 take the ones a detached
server can answer (the time-valued formats by shape through the `t` modifier,
`#{uid}` / `#{host_short}` / the path formats against the values the shell can
read independently, the remaining mouse formats as empty outside a mouse key,
and the `*_mode_format` strings), and 1757 takes the eighteen `client_*` formats
through the nested-client technique — comparing the terminal- and user-derived
ones directly and reducing the pid, tty, creation time, byte counters and the
version-carrying `client_termtype` to their shape.

Cases **1758–1763** apply the same measurement to the hooks, which the options
sweep missed because `OPTIONS_TABLE_HOOK` does not spell its entries the way the
other options are spelled. 37 of the 57 hooks had no case. 1758 arms all
twenty-six `after-<command>` hooks at once, runs each command, and prints which
fired — twenty-five do, and `after-refresh-client` does not, because with no
client that command errors before its hook. The rest take `after-queue` and
`after-set-hook` (which fires for the command that arms hooks, its own included),
`command-error` (with `#{hook}` naming itself, and staying quiet when the command
succeeds), the client lifecycle through the nested-client technique
(`client-attached`, `client-resized` on a real resize, `client-detached`, and
`client-active`/`client-focus-in`/`client-focus-out` staying quiet with nothing
reporting focus), and the theme hooks.

**Every entry of all four tables is now named by at least one case: 195 format
variables, 180 options, 95 copy-mode commands, 57 hooks.** That is coverage of
the *names*, not of every behaviour behind them, and it is measured the way the
blocks above were built — by diffing each table in the C against the corpus,
which is a check worth re-running whenever upstream adds to one.

Cases **1764–1791** are the last block of this round and the one that paid best.
It went after the tables' *contents* rather than their names — usage strings, the
input parser, the modifiers that need a client, and the options whose behaviour
nothing asserted — and turned up four divergences:

- **thirteen usage strings had drifted** (1791) — the `choose-tree` family had
  lost `-k` (and `-h`/`-i`), `break-pane` described `-x`/`-y`/`-X`/`-Y` with the
  wrong words, `command-prompt` was missing `-F`/`-N`/`-P`, `display-menu` and
  `display-popup` had lost a space before `[-T title]`, `send-keys` and
  `send-prefix` showed `-t` as required, `server-access` had dropped its `-t`
  altogether, and `bind-key`, `new-session`, `respawn-pane`, `respawn-window`,
  `set-buffer` and `show-hooks` each ended with the wrong optional argument. A
  usage string is data, so nothing had ever compared them; case 1791 now diffs
  the whole `list-commands` output, excluding only the six lines that are
  supposed to differ (ztmux's five `list-*` commands document their
  structured-output flags, and `znative` exists only here).
- **`#{L:…}` did not set its loop variables** (1778) — `format_loop_clients`
  skipped the `loop_index` / `loop_last_flag` pair the C adds
  (`format.c:5075-5076`), which the session, window and pane loops all have.
- **`#{I/c:…}` / `#{I/f:…}` / `#{I/e:…}` were unimplemented** (1776, 1777) — the
  client-information modifier was missing from the tokenizer, the modifier parse
  and the apply step, along with the two helpers it needs
  (`tty_term_has_name`, `tty_feature_present`). Asking a client about a
  capability or a feature expanded to nothing instead of `1`/`0`.
- **`alert-activity` fired repeatedly for the same window** (1784) —
  `alerts_check_activity` was missing the C's `if (wl->flags & WINLINK_ACTIVITY)
  continue;` (`alerts.c:151`), so the hook fired on every alert pass while the
  flag stood rather than once per transition. The bell check deliberately has no
  such guard, which is why only the activity one needed it.

The block also covers the input parser (ICH/DCH/ECH/REP, SU/SD, DECSC/DECRC,
DECALN, OSC 4/104), the mirrored layouts, the last four commands with no case at
all (`start-server`, `suspend-client`, `customize-mode`, `find-window`), the
key-name table's function/editing/keypad blocks, the
terminal-feature names, and the options whose behaviour was never asserted:
`base-index`/`pane-base-index` on creation, `default-command` spawning a pane,
`history-limit` capping the scrollback, `synchronize-panes` reaching every pane,
`word-separators` moving where `next-word` lands, and the session environment
arriving in a spawned pane.

A fifth upstream defect turned up here too: `respawn-pane` on a **dead** pane
takes the vendored reference's server down, while this port respawns it and
carries on. Case 1788 stops at that boundary and says so — there is no reference
behaviour to compare, and pinning one side of a crash would be pinning nothing.

Cases **1792–1802** are the interaction block: everything here needs a client,
and the nested-client technique drives it by typing into the OUTER pane, which
is the inner client's terminal. Where the render cases pin what a client draws,
these pin what it does. `command-prompt` takes typed input and substitutes `%1`
and `%%`; `confirm-before` runs its command on `y` and drops it on `n`;
`display-menu` runs the item whose key is pressed and runs nothing on Escape;
`choose-tree` switches to the window the selection lands on; the prefix and
prefix2 keys route the next key through the prefix table (and the same key alone
does not); `status-keys emacs` makes `C-a` in the prompt move to the start of the
line. Two of them drive the MOUSE, which nothing had: a real SGR press/release
selects the pane it lands in (rows taken from the panes' own geometry), and a
wheel-up event opens copy mode through the default `WheelUpPane` binding. Two
more pin sizing and searching: `window-size` cycled through largest/smallest/
latest/manual with two clients of DIFFERENT sizes attached, and `wrap-search` on
and off at the end of the scrollback.

One case was written and then deleted rather than kept: `allow-passthrough`
cannot be observed here. The payload leaves the inner pane, reaches the outer
server as its own passthrough, and — with no real terminal above that — is
consumed rather than drawn, so both settings of the option compare equal at 0.
A case that cannot fail is not a case.

Cases **1803–1807** finish the round on the surfaces that need a second process
rather than only a second client. A **control-mode** client (`-C`) runs in a pane
so `capture-pane` reads the protocol it speaks: the `%begin`/`%end` block around
a command's output, the `%session-changed` notification, and the `%error` block
for an unknown command, with ids and timestamps masked. `wait-for` finally gets
its blocking half — a pane parked in `wait-for ztchan`, observed still parked,
then released by `wait-for -S` — which the five earlier wait-for cases could only
approach from the non-blocking side. The rest take `run-shell -b` and
`if-shell -b` not holding up the queue, session groups sharing their windows
(created in one, visible in the other, current window still per-session, killed
from both), and `switch-client -T` moving a client to another key table so a key
bound there fires with no prefix.

**On timing.** Every case that attaches a client starts two servers and drives
input through one into the other, and the runner allows 15 seconds per case per
binary. Two of the interaction cases were written with 12-second poll loops,
which left nothing for the setup: under load they did not fail, they TRUNCATED —
one binary's output stopped mid-case and the diff was against a partial capture.
The polls are now 5 seconds, and `find-window` with a client was dropped
altogether: its expected output contained the host name, which is not portable
between machines. Its flag-parsing sibling (1765) stays.

Cases **1808–1837** close the round on the flags and modifier combinations that
still had no case anywhere — `refresh-client -f`/`-C`/`-S` with a client,
`detach-client -t`/`-s` with two of them, `display-panes` selecting by key,
`copy-mode -s` showing another pane, `display-popup -E` closing on exit,
`select-window -n`/`-p`/`-l`/`-T`, `set-hook -a`, nested and relative
`source-file`, `terminal-features` reaching a client's `#{I/f:}`,
`select-pane -Z`, `join-pane`, `new-window -c` (including `-c` as a format),
`respawn-window -c`/`-e`, `show-messages -J`/`-T` by shape, `set -U` clearing a
window option from every pane, `paste-buffer -S`, `run-shell -E`,
`attach-session -E`, and a dozen cheap format cases combining modifiers that
each had only single-modifier coverage.

One of them found a divergence that is **not** fixed and is recorded instead:
`join-pane -b` puts the joined pane on the other side of the target from where
the reference puts it (`parity/known_gaps/join_pane_before_placement.sh`, with
the minimal reproduction). `layout_split_pane` and its `SPAWN_BEFORE` handling
are ported line for line and agree for `split-window -b`; the join-pane path in
next-3.7 goes through `layout_get_tiled_cell` (`layout.c:1593`), which this port
does not have, so closing it is a port job rather than a patch. Case 1817 covers
join-pane's other flags and stops short of `-b`, saying so.

**A harness note worth keeping.** Running `cargo clippy` (or anything else that
writes to `target/`) while a suite run is in flight removes `target/debug/ztmux`
from under it: the run then reports every remaining case as a failure with
"No such file or directory" on the port side. 79 cases "failed" that way in one
run here. The numbers are worthless from that point on — rebuild and re-run
rather than trusting them.

Cases **1838–1910** are the last block: the interaction and flag surface the
earlier sweeps had left, driven mostly through the nested-client technique —
vi-mode copy keys typed as keys, `send-keys -K` and `-F` and `-M`, a mouse DRAG
selecting text, `copy-mode -u`/`-e`/`-s`, `load-buffer -`, `if-shell -t`, the
alert routing (`monitor-silence`, `activity-action`, `visual-activity`), the
oversized-window path (`window_bigger`, `window_offset_*`, `refresh-client -R`),
`switch-client -Z`/`-T`, `detach-client -E`, `attach -f`/`-r`, `kill-server`,
`show-options -H`, option-name prefixes and array forms, `move-window`/`swap-window`
across sessions, `link-window` inside a group, `move-pane`'s floating-only rule,
and a further two dozen cheap format cases (integer limits, empty-match
substitution, `##` escaping, combining-character width, nested conditionals).

Two divergences came out of it, and both are recorded rather than papered over:
`join-pane -b` (above) and **clock-mode drawing nothing on the client's screen**
(`parity/known_gaps/clock_mode_client_draw.sh`). The clock is painted by the
client rather than into the pane grid — a server-side capture is empty on both
binaries, which is why nothing had caught it — and with ztmux's own overlay
turned off the reference paints digits where this port paints nothing.
`window_clock_draw_screen` is ported, so the gap is in what reaches the client.
Case 1838 keeps the comparable half: entering the mode, `#{pane_mode}`, the empty
pane grid, cancelling, and the options it reads.

**Case design, learned the hard way.** A case that drives a client must assert
the client attached and stop if it did not. Several here were written to carry on
regardless, and under full-suite load they did not fail — they diverged in HOW FAR
each binary got before the 15s budget, which reads as a divergence and is not one.
Forty-one cases now end their attach poll with a guard that prints one line and
exits; the same applies to polls that wait on a pane's output, which should report
a count by name rather than print a screen that is blank on one side.

Cases **1911–1919** are the coda, and one of them paid: key names with modifiers
round-tripping through `list-keys`, `unbind -a` on a table that never existed,
`set -o` on an array index, multi-line format output, object ids not recycling
after a kill, the window stack and `#{window_stack_index}` after killing the
current window, `select-pane -l` and `#{pane_last}` when the last pane is killed,
window index reuse with `renumber-windows` on and off, and `source-file` on a
directory — which was the one that found something.

- **every failed client-side read was reported as a successful empty one**
  (1914) — `file_read_error_callback` ignored the `what` it was handed and sent
  `error: 0` unconditionally, where the C sends
  `(what & EVBUFFER_ERROR) ? EIO : 0` (`file.c:687`). So `source-file` on a
  directory, on a file with no read permission, or on anything else that opens
  and then fails to read came back quiet with status 0, instead of
  `Input/output error: <path>` and status 1. `-q` does not excuse it either: the
  C only skips a quiet ENOENT.

Cases **1920–1925** come from a last measurement: every command's `args` string
in the C, diffed letter by letter against the corpus. That named 58 commands
carrying at least one flag no case mentioned. The substantive ones are covered
here — `clear-history -H`, `copy-mode -d`, `link-window -a`/`-b`,
`unlink-window -k`, `next-window -a`/`previous-window -a` against a real
activity alert, `last-pane -d`/`-e`/`-Z`, the layout cycling commands with a
target and their `select-layout -n`/`-p` aliases, and `split-window -k`/`-m`.

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
from the reference. **One case remains**: `mouse_scrollbar_locations.sh`. The
scrollbar geometry and drawing are ported, but the `keyc` mouse table is the
older flat six-location enum where the C computes `base + (button << 8) +
location` over 19, so `SCROLLBAR_UP` / `_SLIDER` / `_DOWN` and `CONTROL0`-`9`
have no key code to name and five default root bindings have nothing to attach
to. Closing it is an encoding migration rather than a patch — and it would also
retire the sort in case 1498, which exists only because the flat enum orders
`list-keys` differently from the C's type-shifted one.

`cmd_prompt_in_pane.sh` left this directory when the in-pane prompt was ported;
it is now `parity/cases/1506_command_prompt_in_pane.sh` with the rendering pinned
by `1507_pane_prompt_render.sh`.

```sh
bash parity/run_known_gaps.sh   # "GAP" = still unported (expected); "CLOSED" = ported, promote it
```

The runner is an advisory tripwire — it exits non-zero only when a gap closes
(the feature got ported and its case should move to `parity/cases/`), so it never
reddens CI merely because the gaps still exist. Should the directory ever empty it
exits 2 with `no cases in parity/known_gaps/*.sh`; the script is deliberately
left as-is rather than taught to treat "nothing to measure" as success. See
[`parity/known_gaps/README.md`](known_gaps/README.md) for the full inventory and
proof. These gaps do not count against the 1633/1633 ported surface; they measure
the unbuilt surface beyond it.

## Growing the suite

Add a `.fmt` (one format) or `.sh` (one scenario) file under `parity/cases/`.
Keep them small and single-purpose; number-prefix by category. The sibling
suites scaled this to thousands of cases — the same shape scales here.

To record a newly-found divergence that ztmux does *not* yet match, add a `.sh`
case under `parity/known_gaps/` instead and confirm it with `run_known_gaps.sh`.
