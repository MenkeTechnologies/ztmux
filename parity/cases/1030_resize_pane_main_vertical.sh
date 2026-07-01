# main-vertical, then widen the main pane with an absolute resize (strip layout checksum)
$TM set-window-option -g main-pane-width 30
$TM split-window -d
$TM split-window -d
$TM select-layout main-vertical
$TM resize-pane -t 0 -x 45
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
