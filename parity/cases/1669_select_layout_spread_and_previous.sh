# -E spreads the panes out evenly in the current layout; -o restores the previous
# layout. Read the geometry back through #{pane_height}.
$TM split-window -d
$TM split-window -d
$TM select-layout even-vertical
$TM resize-pane -t 1 -y 4
echo "== after a manual resize =="
$TM list-panes -F '#{pane_index}:#{pane_height}' | sort
$TM select-layout -E; echo "spread rc=$?"
echo "== after -E =="
$TM list-panes -F '#{pane_index}:#{pane_height}' | sort
$TM select-layout -o; echo "previous rc=$?"
echo "== after -o =="
$TM list-panes -F '#{pane_index}:#{pane_height}' | sort
