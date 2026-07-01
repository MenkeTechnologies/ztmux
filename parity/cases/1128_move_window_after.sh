$TM new-window -n a
$TM new-window -n b
$TM move-window -a -s a -t 0
$TM list-windows -F '#{window_index}:#{window_name}'
