# Line-oriented motions: each one computes a row or column from the screen
# geometry (top/middle/bottom of the visible area) or from the line's content
# (its first non-blank, its last non-blank). A port that reads copy mode's own
# screen instead of the pane's grid, or that forgets the scroll offset, lands on
# a different row here while still looking "accepted" — the class of bug 1438
# pinned for the cursor formats.
$TM new-window -d -n lines 'printf "    indented one\nsecond line here\nthird\n\nfifth line\n"; sleep 300'
sleep 1
$TM copy-mode -t lines
for c in history-top start-of-line end-of-line back-to-indentation top-line \
         middle-line bottom-line start-of-line; do
  $TM send-keys -X -t lines "$c"
  $TM display-message -p -t lines "$c #{copy_cursor_y},#{copy_cursor_x}"
done
# goto-line is 1-based over the whole history, and clamps rather than failing.
for n in 1 3 5 999; do
  $TM send-keys -X -t lines goto-line "$n"
  $TM display-message -p -t lines "goto $n -> #{copy_cursor_y},#{copy_cursor_x} off=#{scroll_position}"
done
