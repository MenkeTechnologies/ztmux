# split leaves pane 0 active; selecting pane 1 flips the active flag
$TM split-window -d
$TM select-pane -t 1
$TM list-panes -F '#{pane_index}:#{pane_active}'
