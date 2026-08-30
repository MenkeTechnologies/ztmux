# rotate-window moves the panes round the layout: -D forwards, -U backwards, and
# -Z keeps a zoomed pane zoomed. The pane ids stay with their contents, so
# following an id through the indexes shows which way the rotation went.
$TM split-window -d
$TM split-window -d
$TM list-panes -F '#{pane_index}:#{pane_id}:#{pane_height}' | sort
$TM rotate-window -D; echo "-D rc=$?"
$TM list-panes -F '#{pane_index}:#{pane_id}:#{pane_height}' | sort
$TM rotate-window -U; echo "-U rc=$?"
$TM list-panes -F '#{pane_index}:#{pane_id}:#{pane_height}' | sort
