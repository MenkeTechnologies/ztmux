# tiled layout with two panes (strip layout checksum)
$TM split-window -d
$TM select-layout tiled
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
