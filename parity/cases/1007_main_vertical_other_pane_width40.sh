# main-vertical honoring other-pane-width for the non-main column (strip layout checksum)
$TM set-window-option -g main-pane-width 30
$TM set-window-option -g other-pane-width 40
$TM split-window -d
$TM split-window -d
$TM select-layout main-vertical
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
