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
1080/1080 ported surface — they are unbuilt surface.

## The cases

Cases graduate out of this bucket as the feature lands: when a gap starts
matching next-3.7 byte-for-byte, the runner flags it `CLOSED` and it is promoted
to `parity/cases/` (the blocking gate) and deleted here. `pane_zoomed_flag`,
`session_*_flag`, the terminal-feature flags, `codepoint-widths` /
`variation-selector-always-wide`, `default-client-command`, `get-clipboard`, and
the `theme` / `dark-theme-*` / `light-theme-*` palette options (plus `themeX`
colour-name parsing) have already made that trip.

| Case | Feature gap | Unported area |
| --- | --- | --- |
| `opt_pane_scrollbars.sh` | `pane-scrollbars*` (4) | `screen-redraw.c` scrollbar scene (`redraw_draw_scrollbar_span`, `redraw_pane_scrollbar`) |
| `opt_copy_mode_line_numbers.sh` | `copy-mode-line-numbers` + styles (6) | `window-copy.c` line numbers / position |
| `opt_pane_border_status.sh` | `pane-status-*` / `session-status-*` / `window-pane-status-format` (6) | `screen-redraw.c` pane border status |
| `opt_prompt_cursor.sh` | `prompt-cursor-*` / `prompt-command-cursor-*` / `message-format` (5) | `status.c` prompt cursor |
| `opt_tree_mode.sh` | `tree-mode-preview-*` / `tree-mode-*-style` / `switch-mode-match-style` (5) | `mode-tree.c`, `window-tree.c` |
| `opt_misc.sh` | `status-format[2]` (session-status "S:" line) | session-status subsystem (`#{S:...}`, `session-status-*`, `#{session_alert}`) |
| `fmt_floating_pane.sh` | `pane_floating_flag` / `pane_x` / `pane_y` / `pane_z` / `pane_pb_*` | floating panes (`new-pane`) |
| `cmd_switch_mode.sh` | `switch-mode` command | `cmd-switch-mode` |

## Sample proof

```
opt_pane_scrollbars.sh
  next-3.7:  pane-scrollbars off / pane-scrollbars-position right / …
  ztmux   :  invalid option: pane-scrollbars / invalid option: … 

fmt_floating_pane.sh
  next-3.7:  0|0|0|1|hidden|0
  ztmux   :  |||||          (every var expands empty)

cmd_switch_mode.sh
  next-3.7:  command switch-mode: unknown flag -h
  ztmux   :  unknown command: switch-mode
```
