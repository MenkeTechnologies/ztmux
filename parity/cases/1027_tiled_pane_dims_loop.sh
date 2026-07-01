# width x height of every pane in a two-by-two tiled layout
$TM split-window -d
$TM split-window -d
$TM split-window -d
$TM select-layout tiled
$TM display-message -p '#{P:#{pane_index}=#{pane_width}x#{pane_height} }'
