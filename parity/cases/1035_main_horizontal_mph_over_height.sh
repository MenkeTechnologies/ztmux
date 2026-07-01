# main-pane-height larger than the window clamps to the available rows (strip layout checksum)
$TM set-window-option -g main-pane-height 40
$TM split-window -d
$TM select-layout main-horizontal
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
