# even-vertical split of four panes; 24 rows is not evenly divisible (strip layout checksum)
$TM split-window -d
$TM split-window -d
$TM split-window -d
$TM select-layout even-vertical
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
