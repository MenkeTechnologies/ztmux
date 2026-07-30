# Cursor motions at the edges of the grid. Every one of these decrements or
# increments an UNSIGNED coordinate, which is where the previous-prompt crash
# came from (an `x - 1` at x == 0 that the C's guards keep unreachable but a
# literal port evaluates anyway). Driving each motion past its boundary is the
# cheapest way to catch that: in C the cursor simply stops, in a bad port the
# server dies and the display-message below has nothing to answer it.
$TM new-window -d -n edges 'printf "ab\ncd\n"; sleep 300'
sleep 1
$TM copy-mode -t edges
$TM send-keys -X -t edges history-top
$TM send-keys -X -t edges start-of-line
# Left/up at the very first cell: both are no-ops, not underflows.
for c in cursor-left cursor-left cursor-up cursor-up; do
  $TM send-keys -X -t edges "$c"
  $TM display-message -p -t edges "$c #{copy_cursor_y},#{copy_cursor_x}"
done
# Right past the last populated column, then down past the last row.
for c in cursor-right cursor-right cursor-right cursor-right cursor-right; do
  $TM send-keys -X -t edges "$c"
  $TM display-message -p -t edges "$c #{copy_cursor_y},#{copy_cursor_x}"
done
$TM send-keys -X -t edges history-bottom
for c in cursor-down cursor-down cursor-down; do
  $TM send-keys -X -t edges "$c"
  $TM display-message -p -t edges "$c #{copy_cursor_y},#{copy_cursor_x}"
done
# The mode must still be live after all of that.
$TM display-message -p -t edges "alive #{pane_mode} #{pane_in_mode}"
