# swap-pane -D and -U swap with the next and previous pane; -d keeps the active
# pane where it was.
$TM split-window -d
$TM split-window -d
$TM select-pane -t 0
$TM list-panes -F '#{pane_index}:#{pane_height}:#{pane_active}' | sort
$TM swap-pane -D; echo "-D rc=$?"
$TM list-panes -F '#{pane_index}:#{pane_height}:#{pane_active}' | sort
$TM swap-pane -U -d; echo "-U -d rc=$?"
$TM list-panes -F '#{pane_index}:#{pane_height}:#{pane_active}' | sort
