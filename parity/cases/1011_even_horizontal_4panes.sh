# even-horizontal split of four panes; exercises remainder distribution (strip layout checksum)
$TM split-window -d
$TM split-window -d
$TM split-window -d
$TM select-layout even-horizontal
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
