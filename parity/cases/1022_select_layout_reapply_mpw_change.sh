# re-applying main-vertical after changing main-pane-width should pick up the new width (strip layout checksum)
$TM set-window-option -g main-pane-width 30
$TM split-window -d
$TM split-window -d
$TM select-layout main-vertical
$TM set-window-option -g main-pane-width 60
$TM select-layout main-vertical
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
