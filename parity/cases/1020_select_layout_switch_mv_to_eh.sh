# switching layouts by name: main-vertical then even-horizontal on the same panes (strip layout checksum)
$TM set-window-option -g main-pane-width 30
$TM split-window -d
$TM split-window -d
$TM select-layout main-vertical
$TM select-layout even-horizontal
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
