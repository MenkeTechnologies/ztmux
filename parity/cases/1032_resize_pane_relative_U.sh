# even-vertical, then shrink pane 0 by a relative up resize (strip layout checksum)
$TM split-window -d
$TM select-layout even-vertical
$TM resize-pane -t 0 -U 3
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
