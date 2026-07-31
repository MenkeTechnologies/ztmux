# switch-mode: the command surface of next-3.7's `switch-mode` (cmd-choose-tree.c
# dispatching to window-switch.c).
#
# This is the non-drawn half — the flag set, the usage line, the default key
# bindings that reach it, and the fact that entering the mode leaves the pane
# alive with `switch-mode` as its mode. 1489 covers the picker as drawn.
$TM list-commands switch-mode

# The flag set is exactly "F:kst:wZ": anything else is refused by name.
for f in -h -q -N -O -y -i; do
  printf '== %s: ' "$f"
  $TM switch-mode "$f" 2>&1 | head -1
done

# -s/-w/-Z/-k/-F/-t are accepted, and a template may follow.
printf '== accepted: '
$TM switch-mode -s -Z -F '#{session_name}' -t '%0' 'switch-client -t %%' 2>&1 | head -1
printf '== mode is: %s\n' "$($TM display-message -p -t '%0' '#{pane_mode}')"
printf '== in mode: %s\n' "$($TM display-message -p -t '%0' '#{?pane_in_mode,yes,no}')"

# -w selects the window flavour of the same mode.
$TM send-keys -t '%0' -X cancel
$TM switch-mode -w 2>&1 | head -1
printf '== window mode is: %s\n' "$($TM display-message -p -t '%0' '#{pane_mode}')"
$TM send-keys -t '%0' -X cancel
printf '== after cancel: %s\n' "$($TM display-message -p -t '%0' '#{?pane_in_mode,yes,no}')"

# The two default bindings that open it. Taken out of the full table rather than
# asked for by key: `list-keys <key>` pads its flag column differently in ztmux,
# which is a list-keys matter and not a switch-mode one.
$TM list-keys -T prefix | grep switch-mode

# switch-mode-match-style is a window/pane option read only by this mode; it
# takes a style and rejects a non-style.
$TM show-options -wg switch-mode-match-style
$TM set-option -wg switch-mode-match-style 'fg=red,bg=black'
$TM show-options -wg switch-mode-match-style
printf '== bad style: '
$TM set-option -wg switch-mode-match-style 'not-a-style' 2>&1 | head -1
