# tiled-layout-max-columns forces the column count in the 'tiled' layout.
$TM split-window "sleep 300"
$TM split-window "sleep 300"
$TM split-window "sleep 300"
$TM set-option -w tiled-layout-max-columns 1
$TM select-layout tiled
$TM display-message -p '#{window_layout}'
