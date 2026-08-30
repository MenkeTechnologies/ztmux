# -L makes new-pane build a TILED pane instead of a floating one
# (cmd-split-window.c:95-98), so the split flags -h/-v/-l/-b/-f apply and the
# pane is not floating.
$TM set -g status off
echo "start: $($TM list-panes | wc -l | tr -d ' ') pane, $($TM display-message -p '#{window_width}x#{window_height}')"
$TM new-pane -d -L -E; echo "rc=$?"
$TM list-panes -F '  #{pane_index} #{pane_width}x#{pane_height} floating=#{pane_floating_flag}'
echo "== -L -h splits left/right, -l sizes it =="
$TM new-pane -d -L -h -l 20 -E; echo "rc=$?"
$TM list-panes -F '  #{pane_index} #{pane_width}x#{pane_height} floating=#{pane_floating_flag}'
echo "== -L -b puts the new pane before the target =="
$TM new-pane -d -L -b -l 5 -E; echo "rc=$?"
$TM list-panes -F '  #{pane_index} #{pane_left},#{pane_top} #{pane_width}x#{pane_height}'
echo "== without -L the pane floats and the tiled panes keep their sizes =="
before=$($TM list-panes -F '#{pane_width}x#{pane_height}' | tr '\n' ' ')
$TM new-pane -d -E; echo "rc=$?"
after=$($TM list-panes -F '#{?pane_floating_flag,,#{pane_width}x#{pane_height}}' | grep -v '^$' | tr '\n' ' ')
echo "tiled before: $before"
echo "tiled after:  $after"
