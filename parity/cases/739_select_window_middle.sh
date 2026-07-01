# select the middle window of three
$TM new-window
$TM new-window
$TM select-window -t 1
$TM list-windows -F '#{window_index}:#{window_active}'
