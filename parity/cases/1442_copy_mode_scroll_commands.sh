# Scrolling in copy mode moves the view offset and the cursor independently:
# scroll-up/down shift the window over history while pinning the cursor inside
# it, page/halfpage move by a geometry-derived amount, and history-top/bottom
# clamp. #{scroll_position} is the offset the cursor formats have to add back
# in (bug 1438), so it is compared alongside the cursor after every step.
$TM new-window -d -n scr 'i=1; while [ $i -le 60 ]; do echo "row-$i"; i=$((i+1)); done; sleep 300'
sleep 1
$TM copy-mode -t scr
for c in history-top history-bottom scroll-up scroll-up scroll-down halfpage-up \
         halfpage-down page-up page-down scroll-top scroll-middle scroll-bottom; do
  $TM send-keys -X -t scr "$c"
  $TM display-message -p -t scr "$c y=#{copy_cursor_y} off=#{scroll_position} [#{copy_cursor_line}]"
done
# Scrolling past either end clamps instead of wrapping or underflowing.
for c in page-up page-up page-up page-up page-up page-up; do
  $TM send-keys -X -t scr "$c"
done
$TM display-message -p -t scr "topclamp y=#{copy_cursor_y} off=#{scroll_position} [#{copy_cursor_line}]"
for c in page-down page-down page-down page-down page-down page-down; do
  $TM send-keys -X -t scr "$c"
done
$TM display-message -p -t scr "botclamp y=#{copy_cursor_y} off=#{scroll_position} [#{copy_cursor_line}]"
$TM display-message -p -t scr "hist #{history_size} limit=#{history_limit}"
