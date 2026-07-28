# Floating-pane formats: the flag, the pane's window-relative position, and its
# index in the z-index list. Checked on both a floating and a tiled pane.
$TM new-pane -x40 -y10 "sleep 300"
$TM display-message -p 'float #{pane_floating_flag}|#{pane_x}|#{pane_y}|#{pane_z}'
$TM move-pane -P top-left
$TM display-message -p 'moved #{pane_floating_flag}|#{pane_x}|#{pane_y}|#{pane_z}'
$TM display-message -t0 -p 'tiled #{pane_floating_flag}|#{pane_x}|#{pane_y}|#{pane_z}'
