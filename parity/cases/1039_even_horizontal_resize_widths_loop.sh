# relative-resize an even-horizontal layout, then read back each pane width
$TM split-window -d
$TM split-window -d
$TM select-layout even-horizontal
$TM resize-pane -t 1 -R 6
$TM display-message -p '#{P:#{pane_index}=#{pane_width} }'
