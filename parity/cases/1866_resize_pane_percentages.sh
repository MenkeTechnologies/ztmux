# -x and -y take a percentage as well as a count, resolved against the window,
# and the layout clamps what cannot fit.
$TM setw -g window-size manual
$TM resize-window -x 80 -y 24
$TM split-window -d -h
$TM list-panes -F '  #{pane_index}: #{pane_width}x#{pane_height}' | sort
$TM resize-pane -t 0 -x 25%; echo "-x 25% rc=$?"
$TM list-panes -F '  #{pane_index}: #{pane_width}x#{pane_height}' | sort
$TM resize-pane -t 0 -x 75%; echo "-x 75% rc=$?"
$TM list-panes -F '  #{pane_index}: #{pane_width}x#{pane_height}' | sort
echo "== a percentage over 100 is clamped by the layout, not refused =="
$TM resize-pane -t 0 -x 150% 2>&1; echo "rc=$?"
$TM list-panes -F '  #{pane_index}: #{pane_width}' | sort
echo "== and a nonsense size is refused =="
$TM resize-pane -t 0 -x abc 2>&1; echo "rc=$?"
