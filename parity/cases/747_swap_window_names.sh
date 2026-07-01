# swap window 0 with window 2 (all windows explicitly named)
$TM rename-window -t 0 base
$TM new-window -n a
$TM new-window -n b
$TM swap-window -s 0 -t 2
$TM list-windows -F '#{window_index}:#{window_name}'
