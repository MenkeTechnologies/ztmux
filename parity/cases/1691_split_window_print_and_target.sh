# -P prints the new pane, -F formats it, and -t chooses which pane is split.
$TM split-window -d -P -F '#{pane_index}:#{window_name}'
$TM split-window -d -t 0 -P -F '#{pane_index}'
$TM list-panes -F '#{pane_index}:#{pane_height}' | sort
echo "== splitting a pane that does not exist =="
$TM split-window -d -t 99 2>&1; echo "rc=$?"
