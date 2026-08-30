# kill-pane -a kills every pane but the target, and the survivor keeps its own
# index while the layout collapses to one pane.
$TM split-window -d
$TM split-window -d
$TM split-window -d
$TM list-panes -F '#{pane_index}' | sort | tr '\n' ' '; echo
$TM kill-pane -a -t 2; echo "rc=$?"
$TM list-panes -F '#{pane_index}:#{pane_height}' | sort
