# resize-pane on a floating pane: absolute -x/-y (incl. percentages) and
# relative -L/-R/-U/-D, plus the out-of-range error path.
$TM new-pane -x40 -y10 "sleep 300"
$TM display-message -p "start #{pane_floating_flag} #{pane_width}x#{pane_height}@#{pane_x},#{pane_y}"
$TM resize-pane -x30 -y8
$TM display-message -p "abs #{pane_width}x#{pane_height}@#{pane_x},#{pane_y}"
$TM resize-pane -x50% -y50%
$TM display-message -p "percent #{pane_width}x#{pane_height}@#{pane_x},#{pane_y}"
$TM resize-pane -R6
$TM display-message -p "wider #{pane_width}x#{pane_height}@#{pane_x},#{pane_y}"
$TM resize-pane -D2
$TM display-message -p "taller #{pane_width}x#{pane_height}@#{pane_x},#{pane_y}"
# -L/-U grow against the anchored edge, so the offset moves with the size.
$TM resize-pane -L3
$TM display-message -p "left #{pane_width}x#{pane_height}@#{pane_x},#{pane_y}"
$TM resize-pane -U1
$TM display-message -p "up #{pane_width}x#{pane_height}@#{pane_x},#{pane_y}"
$TM resize-pane -x99999
$TM resize-pane -L99999
$TM display-message -p "final #{pane_width}x#{pane_height}@#{pane_x},#{pane_y}"
