# main-vertical with main-pane-width 50 across four panes (strip layout checksum)
$TM set-window-option -g main-pane-width 50
$TM split-window -d
$TM split-window -d
$TM split-window -d
$TM select-layout main-vertical
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
