# select pane 1 then back to pane 0
$TM split-window -d
$TM select-pane -t 1
$TM select-pane -t 0
$TM list-panes -F '#{pane_index}:#{pane_active}'
