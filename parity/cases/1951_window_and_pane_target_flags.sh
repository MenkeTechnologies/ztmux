# The -t of the commands that take nothing else: next-window, previous-window,
# last-window and rotate-window take a session, last-pane a window. Each is
# exercised against a named target rather than the current one, together with
# the error each gives when there is nothing to go back to.
$TM set -g automatic-rename off
$TM set -g status off
$TM new-session -d -s other -n first 'sleep 300'
$TM new-window -d -t other -n second 'sleep 300'
$TM new-window -d -t other -n third 'sleep 300'
cur() { $TM display-message -p -t other '#{window_name}'; }
echo "current in other: $(cur)"
$TM select-window -t other:1; echo "select rc=$? -> $(cur)"
$TM next-window -t other; echo "next rc=$? -> $(cur)"
$TM next-window -t other; echo "next rc=$? -> $(cur)"
$TM previous-window -t other; echo "previous rc=$? -> $(cur)"
$TM last-window -t other; echo "last rc=$? -> $(cur)"
echo "== rotate-window -t moves the panes of the target window =="
$TM split-window -d -t other:first 'sleep 300'
$TM list-panes -t other:first -F '  #{pane_index} #{pane_id}'
$TM rotate-window -t other:first; echo "rotate rc=$?"
$TM list-panes -t other:first -F '  #{pane_index} #{pane_id}'
$TM rotate-window -D -t other:first; echo "rotate -D rc=$?"
$TM list-panes -t other:first -F '  #{pane_index} #{pane_id}'
echo "== last-pane -t names the window =="
$TM select-pane -t other:first.1
$TM last-pane -t other:first; echo "rc=$? active=$($TM display-message -p -t other:first '#{pane_index}')"
$TM last-pane -t other:first; echo "rc=$? active=$($TM display-message -p -t other:first '#{pane_index}')"
echo "== a window with no last pane =="
$TM new-window -d -t other -n solo 'sleep 300'
$TM last-pane -t other:solo 2>&1; echo "rc=$?"
echo "== a session with no last window =="
$TM new-session -d -s fresh -n only 'sleep 300'
$TM last-window -t fresh 2>&1; echo "rc=$?"
