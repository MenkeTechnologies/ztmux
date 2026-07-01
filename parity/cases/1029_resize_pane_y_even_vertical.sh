# even-vertical, then resize pane 0 to an absolute height (strip layout checksum)
$TM split-window -d
$TM select-layout even-vertical
$TM resize-pane -t 0 -y 8
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
