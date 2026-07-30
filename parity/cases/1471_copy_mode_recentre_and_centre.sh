# recentre-top-bottom cycles the view between middle, top and bottom for one
# cursor line, and restarts the cycle whenever the cursor moves to a different
# line of the backing grid. Each step adjusts the cursor row by the SIGNED
# change in the scroll offset, which the C carries as wrapping unsigned
# arithmetic — the exact shape that has taken this server down before.
# cursor-centre-vertical/horizontal move the cursor rather than the view.
$TM new-window -d -n rc 'i=1; while [ $i -le 60 ]; do echo "row-$i"; i=$((i+1)); done; sleep 300'
sleep 1
$TM copy-mode -t rc
at() { $TM display-message -p -t rc "$1 y=#{copy_cursor_y},#{copy_cursor_x} off=#{scroll_position} [#{copy_cursor_line}]"; }
$TM send-keys -X -t rc goto-line 20
at start
# Two full cycles from the same line: middle, top, bottom, middle, top, bottom.
for _ in 1 2 3 4 5 6; do
  $TM send-keys -X -t rc recentre-top-bottom
  at recentre
done
# Moving the cursor to another line restarts the cycle at middle.
$TM send-keys -X -t rc cursor-up
$TM send-keys -X -t rc recentre-top-bottom
at after-move
# From the top of the history, where scrolling up cannot move the view.
$TM send-keys -X -t rc history-top
for _ in 1 2 3; do
  $TM send-keys -X -t rc recentre-top-bottom
  at at-top
done
# And from the very bottom.
$TM send-keys -X -t rc history-bottom
for _ in 1 2 3; do
  $TM send-keys -X -t rc recentre-top-bottom
  at at-bottom
done
# The centre commands move the cursor inside the pane, not the view.
$TM send-keys -X -t rc goto-line 30
$TM send-keys -X -t rc cursor-centre-vertical; at centre-v
$TM send-keys -X -t rc cursor-centre-horizontal; at centre-h
$TM send-keys -X -t rc begin-selection
$TM send-keys -X -t rc cursor-centre-vertical
$TM display-message -p -t rc "sel s=#{selection_start_y},#{selection_start_x} e=#{selection_end_y},#{selection_end_x}"
