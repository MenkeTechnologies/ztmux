# tiled layout with four panes (a clean two-by-two grid) (strip layout checksum)
$TM split-window -d
$TM split-window -d
$TM split-window -d
$TM select-layout tiled
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
