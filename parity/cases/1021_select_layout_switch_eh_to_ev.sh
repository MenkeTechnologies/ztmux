# switching layouts by name: even-horizontal then even-vertical on the same panes (strip layout checksum)
$TM split-window -d
$TM split-window -d
$TM select-layout even-horizontal
$TM select-layout even-vertical
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
