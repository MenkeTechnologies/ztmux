# A floating pane keeps its size when the tiled layout is resized. It sits
# outside the tiled flow, so layout_resize_adjust and layout_resize_child_cells
# must skip it; otherwise attaching a client (which takes a row for the status
# line) shrank the float by one row and its border box no longer matched the
# pane, drawing the bottom border inside the pane.
geom() { $TM list-panes -F '#{pane_index} #{pane_width}x#{pane_height}@#{pane_x},#{pane_y} f=#{pane_floating_flag}'; echo "-- $1"; }
$TM split-window -h -d "sleep 300"
$TM new-pane -d -x24 -y6 "sleep 300"
geom start
# Shrink and grow the window: the float must not change.
$TM resize-window -y 23
geom window-23
$TM resize-window -y 20
geom window-20
$TM resize-window -x 60
geom window-60x20
$TM resize-window -x 80 -y 24
geom back-to-80x24
# Resizing a tiled pane must not disturb the float either.
$TM resize-pane -t0 -x 30
geom tiled-resized
