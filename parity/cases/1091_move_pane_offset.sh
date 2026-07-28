# move-pane -X/-Y absolute and -U/-L/-R relative offsets on a floating pane.
# NB: upstream's args template has no -D even though the usage lists it, so a
# relative move down is rejected; ztmux matches that.
$TM new-pane -x40 -y10 "sleep 300"
$TM move-pane -X10 -Y4
$TM display-message -p "abs #{pane_x},#{pane_y}"
$TM move-pane -R5
$TM display-message -p "right #{pane_x},#{pane_y}"
$TM move-pane -U2 -L4
$TM display-message -p "up-left #{pane_x},#{pane_y}"
$TM move-pane -R
$TM display-message -p "default-adjust #{pane_x},#{pane_y}"
$TM move-pane -X50% -Y25%
$TM display-message -p "percent #{pane_x},#{pane_y}"
$TM move-pane -D3
$TM move-pane -Rbogus
$TM move-pane -t0 -X1
