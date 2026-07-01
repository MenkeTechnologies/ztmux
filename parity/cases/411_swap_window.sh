$TM new-window -n a
$TM new-window -n b
$TM swap-window -s 1 -t 2
$TM list-windows -F '#{window_index}:#{window_name}'
