# selecting window 0 makes it the active window
$TM new-window
$TM select-window -t 0
$TM list-windows -F '#{window_index}:#{window_active}'
