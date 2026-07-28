# A floating pane may be positioned partly past an edge, which gives it a
# NEGATIVE offset. tmux.h:1276 and 1518 type layout_cell and window_pane
# xoff/yoff as int for exactly this; stored unsigned, -9 read back as
# 4294967287 and every downstream comparison overflowed.
geom() { $TM list-panes -F '#{pane_index} #{pane_width}x#{pane_height}@#{pane_x},#{pane_y} f=#{pane_floating_flag}'; echo "-- $1"; }
edges() { $TM display-message -p "$1 L=#{pane_left} T=#{pane_top} R=#{pane_right} B=#{pane_bottom}"; }
$TM split-window -h -d "sleep 300"
$TM new-pane -x30 -y8 "sleep 300"
geom start
edges start
# Past the top-left corner.
$TM move-pane -X-10 -Y-4
geom neg-both
edges neg-both
# Past the left only, then the top only.
$TM move-pane -X-20 -Y5
geom neg-x
edges neg-x
$TM move-pane -X15 -Y-6
geom neg-y
edges neg-y
# Well past the bottom-right.
$TM move-pane -X75 -Y22
geom past-br
edges past-br
# Relative moves back across zero.
$TM move-pane -X0 -Y0
$TM move-pane -L5 -U3
geom relative-across-zero
edges relative-across-zero
