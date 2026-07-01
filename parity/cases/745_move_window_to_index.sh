# move a window to a specific free index
$TM new-window
$TM move-window -t 4
$TM list-windows -F '#{window_index}'
