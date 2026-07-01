# main-horizontal honoring other-pane-height for the non-main row (strip layout checksum)
$TM set-window-option -g main-pane-height 10
$TM set-window-option -g other-pane-height 6
$TM split-window -d
$TM split-window -d
$TM select-layout main-horizontal
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
