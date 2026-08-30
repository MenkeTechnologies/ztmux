# Window and pane ids are per-server counters that never go back: killing an
# object does not free its id for the next one, which is what makes an id a
# stable handle where an index is not.
$TM set -g automatic-rename off
$TM list-windows -F 'start window #{window_id}' | sort
$TM new-window -d -n first
$TM new-window -d -n second
$TM list-windows -F '  #{window_name}=#{window_id}' | sort
$TM kill-window -t first
$TM new-window -d -n third
echo "after killing first and making third:"
$TM list-windows -F '  #{window_name}=#{window_id}' | sort
echo "== the same for panes =="
$TM split-window -d -t third
$TM list-panes -t third -F '  pane #{pane_index}=#{pane_id}' | sort
$TM kill-pane -t third.1
$TM split-window -d -t third
$TM list-panes -t third -F '  pane #{pane_index}=#{pane_id}' | sort
