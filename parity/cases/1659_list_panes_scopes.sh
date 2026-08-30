# -s lists every pane in the session and -a every pane on the server; -f filters
# and -F formats.
$TM new-window -d -n second
$TM split-window -d -t second
echo "== current window =="
$TM list-panes -F '#{window_name}:#{pane_index}' | sort
echo "== session (-s) =="
$TM list-panes -s -F '#{window_name}:#{pane_index}' | sort
echo "== server (-a) =="
$TM list-panes -a -F '#{window_name}:#{pane_index}' | sort
echo "== filtered to the active pane =="
$TM list-panes -s -f '#{pane_active}' -F '#{window_name}:#{pane_index}' | sort
