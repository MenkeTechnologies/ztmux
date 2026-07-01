$TM new-window -n a
$TM new-window -n b
$TM new-window -n c
$TM swap-window -s 1 -t 3
$TM list-windows -F '#{window_index}:#{window_name}'
