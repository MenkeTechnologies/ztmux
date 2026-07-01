# per-pane heights of an even-vertical layout across three panes
$TM split-window -d
$TM split-window -d
$TM select-layout even-vertical
$TM display-message -p '#{P:#{pane_index}=#{pane_height} }'
