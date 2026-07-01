$TM new-window -n a
$TM new-window -n b
$TM move-window -b -s b -t 1
$TM list-windows -F '#{window_index}:#{window_name}'
