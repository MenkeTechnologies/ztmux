# floating pane geometry: half width, quarter height, cascade offset
$TM new-pane -d "sleep 300"
$TM list-panes -F '#{pane_index} #{pane_width}x#{pane_height}@#{pane_left},#{pane_top}'
