# The layout string is the layout tree serialised with a checksum, so feeding
# one back in has to rebuild the identical tree: same cell sizes, same offsets,
# same order. Offsets inside those cells are signed in the C and were typed
# unsigned in the port once already, which shows up here as a cell that lands
# one column off after a round-trip even though the first serialisation looked
# right.
$TM new-window -d -n lay 'sleep 300'
$TM split-window -d -t lay 'sleep 300'
$TM split-window -d -h -t lay 'sleep 300'
$TM split-window -d -t lay.0 'sleep 300'
saved=$($TM display-message -p -t lay '#{window_layout}')
echo "layout=$saved"
$TM list-panes -t lay -F '#{pane_index} #{pane_width}x#{pane_height} @#{pane_left},#{pane_top}-#{pane_right},#{pane_bottom}'
# Round-trip: switch to another layout, then restore the saved string.
$TM select-layout -t lay even-vertical
$TM list-panes -t lay -F 'even #{pane_index} #{pane_width}x#{pane_height} @#{pane_left},#{pane_top}'
$TM select-layout -t lay "$saved"
echo "restored=$($TM display-message -p -t lay '#{window_layout}')"
$TM list-panes -t lay -F 'back #{pane_index} #{pane_width}x#{pane_height} @#{pane_left},#{pane_top}-#{pane_right},#{pane_bottom}'
# A corrupt checksum and a malformed string are both rejected.
$TM select-layout -t lay 'ffff,80x24,0,0,0' 2>&1
$TM select-layout -t lay 'not-a-layout' 2>&1
echo "still=$($TM display-message -p -t lay '#{window_layout}')"
