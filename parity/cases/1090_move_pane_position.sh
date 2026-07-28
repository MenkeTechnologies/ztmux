# move-pane -P: place a floating pane at each named position. The new pane is
# left active (no -d) so it is move-pane's default target.
$TM new-pane -x40 -y10 "sleep 300"
$TM display-message -p "floating=#{pane_floating_flag} #{pane_width}x#{pane_height}"
for p in top-left top-centre top-center top-right centre-left center-left \
         centre center centre-right center-right bottom-left bottom-centre \
         bottom-right top-left-centre top-right-centre bottom-left-centre \
         bottom-right-centre; do
  $TM move-pane -P "$p"
  $TM display-message -p "$p #{pane_x},#{pane_y}"
done
$TM move-pane -P nonsense
# A borderless floating pane has no border row/column to skip, so the corner
# positions sit flush against the window edge.
$TM set -p pane-border-lines none
for p in top-left bottom-right centre; do
  $TM move-pane -P "$p"
  $TM display-message -p "noborder-$p #{pane_x},#{pane_y}"
done
