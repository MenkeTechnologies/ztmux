# tiled layout with five panes; ragged final row exercises remainder handling (strip layout checksum)
$TM split-window -d
$TM split-window -d
$TM split-window -d
$TM split-window -d
$TM select-layout tiled
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
