# -f splits the full window width/height rather than just the current pane, and
# -Z zooms the new pane as it is created.
$TM split-window -d -h
$TM split-window -d -f -l 4
$TM list-panes -F '#{pane_index} #{pane_left},#{pane_top} #{pane_width}x#{pane_height}' | sort
echo "== -Z zooms the new pane =="
$TM split-window -d -Z
$TM display-message -p 'zoomed=#{window_zoomed_flag}'
$TM resize-pane -Z
$TM display-message -p 'unzoomed=#{window_zoomed_flag}'
