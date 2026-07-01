# from the last window, next-window wraps to window 0
$TM new-window
$TM new-window
$TM next-window
$TM list-windows -F '#{window_index}:#{window_active}'
