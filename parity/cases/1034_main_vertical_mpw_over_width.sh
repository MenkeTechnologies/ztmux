# main-pane-width larger than the window clamps to the available columns (strip layout checksum)
$TM set-window-option -g main-pane-width 100
$TM split-window -d
$TM select-layout main-vertical
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
