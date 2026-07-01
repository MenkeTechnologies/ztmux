# tiled layout should preserve the pane count; report indices after applying it
$TM split-window -d
$TM split-window -d
$TM split-window -d
$TM select-layout tiled
$TM list-panes -F '#{pane_index}'
