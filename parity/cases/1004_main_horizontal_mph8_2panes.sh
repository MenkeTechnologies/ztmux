# main-horizontal with main-pane-height 8 across two panes (strip layout checksum)
$TM set-window-option -g main-pane-height 8
$TM split-window -d
$TM select-layout main-horizontal
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
