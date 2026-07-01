# main-horizontal with main-pane-height 10 across three panes (strip layout checksum)
$TM set-window-option -g main-pane-height 10
$TM split-window -d
$TM split-window -d
$TM select-layout main-horizontal
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
