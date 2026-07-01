$TM split-window -h
$TM select-pane -t 0
$TM swap-pane -s 0 -t 1
$TM list-panes -F '#{pane_index}'
