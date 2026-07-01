# per-pane widths of an even-horizontal layout across four panes
$TM split-window -d
$TM split-window -d
$TM split-window -d
$TM select-layout even-horizontal
$TM display-message -p '#{P:#{pane_index}=#{pane_width} }'
