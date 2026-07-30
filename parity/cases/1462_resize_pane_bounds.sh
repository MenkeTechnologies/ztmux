# resize-pane arithmetic runs in both directions against a neighbour that has a
# minimum size, so every adjustment is a clamp against a difference of two
# offsets. Asking for more than the window can give, or for a negative-sized
# pane, is where a signed/unsigned mistype turns a clamp into a wrap — the pane
# and layout-cell offsets the port already had to re-type as signed.
$TM new-window -d -n rz 'sleep 300'
$TM split-window -d -t rz 'sleep 300'
$TM split-window -d -h -t rz 'sleep 300'
dump() { $TM list-panes -t rz -F "$1 #{pane_index} #{pane_width}x#{pane_height} @#{pane_left},#{pane_top}"; }
dump start
for spec in "-U 3" "-D 5" "-L 4" "-R 2" "-U 100" "-D 100" "-L 100" "-R 100"; do
  # shellcheck disable=SC2086
  $TM resize-pane -t rz.0 $spec 2>&1
  dump "$spec"
done
# Absolute sizes, including ones the window cannot satisfy.
for spec in "-x 40" "-y 10" "-x 1" "-y 1" "-x 200" "-y 200" "-x 0" "-y 0"; do
  # shellcheck disable=SC2086
  $TM resize-pane -t rz.0 $spec 2>&1
  dump "$spec"
done
# Percentages resolve against the window, and a bad one is an error.
$TM resize-pane -t rz.0 -x 50% 2>&1; dump pct
$TM resize-pane -t rz.0 -x 150% 2>&1; dump pct-over
$TM resize-pane -t rz.0 -x abc 2>&1
$TM display-message -p -t rz 'layout=#{window_layout}'
