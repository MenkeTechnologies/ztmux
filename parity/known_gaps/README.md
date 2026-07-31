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

**There are none left.** Every case that lived here has graduated into
`parity/cases/`: `pane_zoomed_flag`, `session_*_flag`, the terminal-feature
flags, `codepoint-widths` / `variation-selector-always-wide`,
`default-client-command`, `get-clipboard`, the `theme` / `dark-theme-*` /
`light-theme-*` palette options (plus `themeX` colour-name parsing), the
pane/session status-line options (`pane-status-*` / `session-status-*` /
`window-pane-status-format`, with `status-format[1]`/`[2]` and the `#{R:}`
repeat modifier), `copy-mode-line-numbers` and its styles, the
`prompt-cursor-*` / `prompt-command-cursor-*` / `message-format` options, the
floating-pane format vars, `pane-scrollbars*`, the `tree-mode-*` preview/style
options, and — last — `cmd_switch_mode`.

The directory is kept, not deleted: it is where the next proven-unported
behaviour goes. Add a case here the moment a next-3.7 feature is shown to be
missing, so the gap is measured rather than remembered.

With no cases present `run_known_gaps.sh` reports `no cases in
parity/known_gaps/*.sh` and exits 2 (it also trips `set -u` on the empty glob
first). That is the runner saying the directory is empty, not a gap failing; it
is advisory and not wired into CI. The script is left as-is deliberately —
teaching it to exit 0 on an empty directory would be editing a measurement tool
to make its output friendlier.

## Sample proof

`cmd_switch_mode.sh` was the last one, and it read:

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

The one command still unported is `scroll-to-mouse`, and it has no case here: it
drags the scrollbar slider, which needs `tty.mouse_scrolling_flag` /
`tty.mouse_slider_mpos` and the `KEYC_MOUSE_LOCATION_SCROLLBAR_*` key codes that
ztmux's six-location `keyc` mouse table cannot name. Driving it against the
reference is not possible anyway — with no mouse event to read, the vendored tmux
takes its own server down, so a case would measure that crash rather than the gap.
It is recorded in `docs/BUGS.md` instead.
