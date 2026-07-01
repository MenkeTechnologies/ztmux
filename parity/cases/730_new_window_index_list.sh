# create two windows, list all window indices deterministically
$TM new-window
$TM new-window
$TM list-windows -F '#{window_index}'
