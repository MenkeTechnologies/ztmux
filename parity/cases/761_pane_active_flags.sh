# after a detached split, pane 0 stays active and pane 1 does not
$TM split-window -d
$TM list-panes -F '#{pane_index}:#{pane_active}'
