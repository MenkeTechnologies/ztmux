# -r renumbers windows to close the gap left by a kill
$TM new-window
$TM new-window
$TM kill-window -t 1
$TM move-window -r
$TM list-windows -F '#{window_index}'
