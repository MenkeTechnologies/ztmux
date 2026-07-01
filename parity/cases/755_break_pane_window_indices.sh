# break-pane moves the active pane into a new window
$TM split-window -d
$TM break-pane -d
$TM list-windows -F '#{window_index}'
