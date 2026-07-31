# switch-mode -k: the pane the mode ran in is killed when the mode exits.
#
# `-k` sets `window_mode_entry.kill` (`window.c:1380`) and
# `window_pane_reset_mode` calls `server_kill_pane` on the way out
# (`window.c:1428`). The default `Tab`/`BTab` bindings open the picker in a
# scratch floating pane and pass `-k`, so without this the scratch pane outlives
# the picker.
#
# A mode only sees keys through an attached client, so this runs a second
# ("inner") server whose client lives in a pane of the outer one — the same
# technique as 1484/1486/1489 — and sends Escape to leave the mode.
set -- $TM
BIN=$1
SOCK=$3
IN="${SOCK}_k"

inner() { $BIN -L "$IN" "$@" >/dev/null 2>&1; }

inner -f /dev/null new-session -d -s S -n w1 -x 60 -y 12 'sleep 300'
inner set-option -g status off
$TM new-session -d -s outer -x 60 -y 12 "$BIN -L $IN attach -t S" >/dev/null 2>&1
sleep 1

inner split-window -d 'sleep 300'
printf '== panes before: %s\n' "$($BIN -L "$IN" list-panes -F '#{pane_index}' 2>&1 | tr '\n' ' ')"

inner switch-mode -k -s -t '%1'
sleep 1
printf '== in mode: %s\n' "$($BIN -L "$IN" display-message -p -t '%1' '#{?pane_in_mode,yes,no}' 2>&1)"
inner send-keys -t '%1' Escape
sleep 1
printf '== panes after -k: %s\n' "$($BIN -L "$IN" list-panes -F '#{pane_index}' 2>&1 | tr '\n' ' ')"

# Without -k the pane survives leaving the mode.
inner split-window -d 'sleep 300'
inner switch-mode -s -t '%2'
sleep 1
inner send-keys -t '%2' Escape
sleep 1
printf '== panes after no -k: %s\n' "$($BIN -L "$IN" list-panes -F '#{pane_index}' 2>&1 | tr '\n' ' ')"

$TM kill-session -t outer >/dev/null 2>&1
inner kill-server
