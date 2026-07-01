$TM new-window -n a
$TM new-window -n b
$TM swap-window -d -s a -t b
$TM list-windows -F '#{window_index}:#{window_name}'
