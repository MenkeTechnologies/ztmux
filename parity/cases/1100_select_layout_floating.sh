# select-layout must leave floating panes alone: they take no place in the
# tiled grid, are not counted when dividing space, and keep their flag, size
# and offsets. Previously every layout rebuilt each pane's leaf cell, which
# folded the float into the tiled layout and cleared its floating flag.
geom() { $TM list-panes -F '#{pane_index} #{pane_width}x#{pane_height}@#{pane_x},#{pane_y} f=#{pane_floating_flag}'; echo "-- $1"; }
$TM split-window -v -d "sleep 300"
$TM new-pane -d -x30 -y8 "sleep 300"
geom start
for l in even-vertical even-horizontal tiled main-vertical main-horizontal; do
  $TM select-layout "$l"
  geom "$l"
done
# A second float, and layouts again.
$TM new-pane -d -x20 -y5 "sleep 300"
$TM select-layout tiled
geom two-floats-tiled
$TM select-layout even-horizontal
geom two-floats-even-h
