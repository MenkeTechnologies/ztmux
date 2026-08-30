# Known gaps — proven-unported next-3.7 behaviour

`parity/cases/` holds behaviour ztmux **matches** byte-for-byte against the
vendored next-3.7 tmux — the green, blocking gate. This directory holds the
inverse: next-3.7 behaviour ztmux does **not** implement yet. Each case is
expected to **diverge** between the reference tmux and ztmux; that divergence is
the proof the feature is unported.

Run them with the inverted runner:

```sh
bash parity/run_known_gaps.sh
```

A case "passes" the gap suite by diverging (`GAP`). If a case ever starts
matching (`CLOSED`), the feature has been ported — promote the case to
`parity/cases/` and delete it here. The runner exits non-zero **only** when a gap
unexpectedly closes, so it can run as an advisory tripwire without going red
merely because the gaps still exist. It is intentionally **not** wired into the
blocking CI parity gate.

These are real next-3.7 features with no ztmux counterpart (verified against the
`next-3.7` reference binary, not the CHANGES text). They are not defects in the
ported surface `parity/cases/` measures — they are unbuilt surface.

## The cases

**One: `clock_mode_client_draw.sh`.** `clock-mode` paints nothing on the
client's screen. The clock is drawn by the CLIENT, through the mode's screen
rather than into the pane's grid, so a server-side `capture-pane` is empty on
both binaries; read back through an attached client — this suite's nested-client
technique — the reference paints the digits and this port paints nothing once
ztmux's own floating overlay (`@ztmux-ratatui`) is off. `window_clock_draw_screen`
is ported, so what is missing is what reaches the client, not the digits.
Entering and leaving the mode, and the options it reads, are compared by
`parity/cases/1838_clock_mode_state.sh`.

`join_pane_before_placement.sh` used to be the other one: `join-pane -b` put the
joined pane on the opposite side of the target. It closed with the port of
`layout_get_tiled_cell` (`layout.c:1593`), which next-3.7's join-pane reaches the
layout through (`cmd-join-pane.c:419`) and this port did not have — it called
`layout_split_pane` directly. The C leaves `cmd_join_pane_exec`'s own `flags` at
zero (`cmd-join-pane.c:379`), so `-b` reaches the layout but never the pane-list
insert. Its case is now `parity/cases/1943_join_pane_before_placement.sh`.

Historically this directory was empty: every gap recorded here before had been
ported and its case promoted to `parity/cases/`. That is still the intended end
state — a gap is added the moment one is
proven, so an empty directory means nothing is currently proven missing, and
says nothing about surface no case has probed yet. `docs/BUGS.md` is where
suspected-but-unproven gaps and open defects live.

The last to graduate was `mouse_scrollbar_locations.sh` — the scrollbar and
control mouse locations. The C names 19 locations per mouse family
(`tmux.h:177-197`) where this tree carried the older six, so `SCROLLBAR_UP` /
`_SLIDER` / `_DOWN` and `CONTROL0`-`9` had no key code, five default root
bindings had nothing to attach to, and `copy-mode -S` was unreachable because
the slider drag is its only default caller. All 19 landed; the case is now
`parity/cases/1511_mouse_scrollbar_locations.sh`, rewritten to pin the names,
the bindings and the flag rather than record their absence, and
`parity/cases/1512_style_range_control.sh` pins the `#[range=control|N]`
directive the CONTROL locations are reached through.

Everything else that lived here graduated into
`parity/cases/` before it: `cmd_prompt_in_pane` (the `command-prompt -P` in-pane prompt,
promoted once `window_pane` gained its prompt fields and `window.c`'s pane-prompt
functions were ported — now `parity/cases/1506_command_prompt_in_pane.sh` and
`1507_pane_prompt_render.sh`), `pane_zoomed_flag`, `session_*_flag`, the terminal-feature
flags, `codepoint-widths` / `variation-selector-always-wide`,
`default-client-command`, `get-clipboard`, the `theme` / `dark-theme-*` /
`light-theme-*` palette options (plus `themeX` colour-name parsing), the
pane/session status-line options (`pane-status-*` / `session-status-*` /
`window-pane-status-format`, with `status-format[1]`/`[2]` and the `#{R:}`
repeat modifier), `copy-mode-line-numbers` and its styles, the
`prompt-cursor-*` / `prompt-command-cursor-*` / `message-format` options, the
floating-pane format vars, `pane-scrollbars*`, the `tree-mode-*` preview/style
options, and `cmd_switch_mode`.

Add a case here the moment a next-3.7 feature is shown to be missing, so the gap
is measured rather than remembered.

With no cases present `run_known_gaps.sh` reports `no cases in
parity/known_gaps/*.sh` and exits 2 (it also trips `set -u` on the empty glob
first). That is the runner saying the directory is empty, not a gap failing; it
is advisory and not wired into CI. The script is left as-is deliberately —
teaching it to exit 0 on an empty directory would be editing a measurement tool
to make its output friendlier.

## Sample proof

`cmd_switch_mode.sh` was the second-to-last to close, and it read:

```
cmd_switch_mode.sh
  next-3.7:  command switch-mode: unknown flag -h
  ztmux   :  unknown command: switch-mode
```

It closed with the `prompt.c` / `window-switch.c` port and is now covered by
`parity/cases/1488_switch_mode.sh` (the command surface),
`parity/cases/1489_switch_mode_draw.sh` (the picker as drawn) and
`parity/cases/1490_switch_mode_kill.sh` (`-k` disposing of the pane), the last
two captured through a nested client. `switch-mode-match-style` had been in the option table with
nothing reading it; `window-switch.c:289` is that reader, so the option is live.

`cmd_copy_mode_missing.sh` and `cmd_capture_pane_flags.sh` lived here until the
copy-mode command table and `capture-pane -F/-H/-L/-M` were ported; they are now
covered by `parity/cases/1471`–`1477`. `opt_pane_scrollbars.sh` graduated with the
pane-scrollbars port and is covered by `parity/cases/1483` (the column a reserved
bar takes) and `1484` (the bar as drawn, captured through a nested client).
`opt_tree_mode.sh` graduated with the tree-mode preview port: `parity/cases/1485`
covers the five options themselves, `1486` the choose-tree session preview as
drawn and `1487` the selection style and the per-pane preview, both captured
through the same nested client.

`scroll-to-mouse` was the last unported copy-mode command and the reason this
directory could not close: naming it needed the `SCROLLBAR_*` key codes, and
driving it needs a slider drag, which no case can synthesise — with no mouse
event to read, the vendored tmux takes its own server down, so such a case would
measure that crash rather than the gap. The command itself came over with the
copy-mode command table (`window-copy.c:1678` -> `src/ported/window_copy.rs:1819`,
table entry at `:4295`), and the key codes came over with the 19 locations, so
neither is a gap any more. It stays uncased for the reason above — that is a
limit of what this harness can drive, recorded here rather than left implied.
