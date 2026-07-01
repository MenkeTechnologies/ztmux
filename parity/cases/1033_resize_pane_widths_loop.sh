# pane widths after a resize on an even-horizontal layout, via the pane loop
$TM split-window -d
$TM split-window -d
$TM select-layout even-horizontal
$TM resize-pane -t 0 -x 20
$TM display-message -p '#{P:#{pane_index}=#{pane_width} }'
