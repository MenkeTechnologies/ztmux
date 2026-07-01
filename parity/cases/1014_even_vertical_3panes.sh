# even-vertical split of three panes; 24 rows split three ways (strip layout checksum)
$TM split-window -d
$TM split-window -d
$TM select-layout even-vertical
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
