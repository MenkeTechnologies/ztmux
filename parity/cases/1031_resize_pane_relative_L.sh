# even-horizontal, then shrink pane 0 by a relative left resize (strip layout checksum)
$TM split-window -d
$TM select-layout even-horizontal
$TM resize-pane -t 0 -L 5
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
