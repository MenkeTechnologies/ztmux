# -l takes a size in lines (or a percentage), -p a percentage, and -b puts the
# new pane before the target. The pane heights show which is which.
$TM split-window -d -l 6
$TM list-panes -F '#{pane_index}:#{pane_height}' | sort
$TM kill-pane -t 1
$TM split-window -d -p 25
$TM list-panes -F '#{pane_index}:#{pane_height}' | sort
$TM kill-pane -t 1
echo "== -b puts the new pane first =="
$TM split-window -d -b -l 5
$TM list-panes -F '#{pane_index}:#{pane_height}' | sort
