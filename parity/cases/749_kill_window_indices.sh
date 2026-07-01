# killing the middle window leaves a gap in the index list
$TM new-window
$TM new-window
$TM kill-window -t 1
$TM list-windows -F '#{window_index}'
