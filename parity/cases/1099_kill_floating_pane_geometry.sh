# Killing a floating pane must not hand its size to a tiled neighbour. A
# floating cell takes no space in the tiled flow, so destroying it is a plain
# removal; treating it as tiled grew the neighbour by the float's size + 1 and
# left panes larger than the window.
geom() { $TM list-panes -F '#{pane_index} #{pane_width}x#{pane_height}@#{pane_x},#{pane_y} f=#{pane_floating_flag}'; echo "-- $1"; }
$TM new-pane -d -x30 -y8 "sleep 300"
geom created
$TM kill-pane -t1
geom killed-float
$TM split-window -v -d "sleep 300"
$TM new-pane -d -x24 -y6 "sleep 300"
geom split-plus-float
$TM kill-pane -t2
geom killed-float-again
# Two floats, removed one at a time.
$TM new-pane -d -x20 -y5 "sleep 300"
$TM new-pane -d -x16 -y4 "sleep 300"
$TM kill-pane -t2
$TM kill-pane -t2
geom two-floats-removed
