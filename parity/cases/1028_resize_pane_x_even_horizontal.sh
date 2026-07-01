# even-horizontal, then resize pane 0 to an absolute width (strip layout checksum)
$TM split-window -d
$TM select-layout even-horizontal
$TM resize-pane -t 0 -x 30
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
