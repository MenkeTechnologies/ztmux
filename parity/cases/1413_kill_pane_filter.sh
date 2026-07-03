# kill-pane -a -f (filter, -a only).
$TM split-window "sleep 300"
$TM split-window "sleep 300"
$TM kill-pane -a -f '#{==:#{pane_index},1}' -t 0
$TM list-panes -F '#{pane_index}'
$TM kill-pane -f x
