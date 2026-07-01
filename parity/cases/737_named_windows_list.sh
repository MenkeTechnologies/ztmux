# give every window an explicit name (avoid the auto-named base window)
$TM rename-window -t 0 base
$TM new-window -n a
$TM new-window -n b
$TM list-windows -F '#{window_index}:#{window_name}'
