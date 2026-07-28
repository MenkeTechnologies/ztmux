# A floating pane overlapping a tiled pane border. The border must stay behind
# the float. This pins the model side; the rendered screen is what actually
# regressed, so the geometry here is the invariant the drawing code reads:
# the float's own extent, its border ring, and the tiled divider column all
# stay put when focus moves off the float and back.
geom() { $TM list-panes -F '#{pane_index} #{pane_width}x#{pane_height}@#{pane_x},#{pane_y} f=#{pane_floating_flag} a=#{pane_active}'; echo "-- $1"; }
$TM split-window -h -d "sleep 300"
$TM new-pane -x30 -y8 "sleep 300"
$TM move-pane -X28 -Y6
geom float-over-divider
$TM select-pane -t0
geom defocused-to-left
$TM select-pane -t1
geom defocused-to-right
$TM select-pane -t2
geom refocused-float
$TM resize-pane -t0 -x 30
geom tiled-resized-while-float-up
