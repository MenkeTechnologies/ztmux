# The word motions are the densest index arithmetic in copy mode: each one walks
# the grid cell by cell classifying characters against word-separators, and the
# stop condition is an off-by-one away from either overshooting the line or
# never moving at all. The search bug (1436) was exactly that shape in a sibling
# loop, so every motion's resulting cursor position is compared here, not just
# whether the command was accepted.
$TM new-window -d -n words 'printf "alpha bravo  charlie\ndelta_echo foxtrot.golf\n  hotel india\n"; sleep 300'
sleep 1
$TM copy-mode -t words
$TM send-keys -X -t words history-top
$TM send-keys -X -t words start-of-line
for c in next-word next-word next-word-end next-word-end next-space next-space-end \
         previous-word previous-word previous-space; do
  $TM send-keys -X -t words "$c"
  $TM display-message -p -t words "$c #{copy_cursor_y},#{copy_cursor_x} [#{copy_cursor_word}]"
done
# Word motions must also cross line boundaries, and stop at the end of the
# pane's content rather than running off it.
$TM send-keys -X -t words history-bottom
$TM send-keys -X -t words start-of-line
for c in next-word next-word next-word next-word next-word next-word; do
  $TM send-keys -X -t words "$c"
  $TM display-message -p -t words "cross #{copy_cursor_y},#{copy_cursor_x}"
done
