# refresh-on/off/toggle drive copy mode's automatic refresh: with it on, a
# timer reconciles the backing screen with the live pane as new output arrives,
# so a view sitting at the bottom follows that output instead of freezing at the
# moment the mode was entered. Without a client attached the timer still runs on
# the server, so the backing grid growing (or not) is the assertion.
$TM new-window -d -n rf 'i=1; while [ $i -le 5 ]; do echo "start-$i"; i=$((i+1)); done; cat; sleep 300'
sleep 1
$TM copy-mode -t rf
$TM display-message -p -t rf "entered in=#{pane_in_mode} y=#{copy_cursor_y} off=#{scroll_position}"
# Refresh off (the default): output written after entering copy mode does not
# appear in the mode's own view of the history.
$TM send-keys -t rf 'aaa' Enter
sleep 1
$TM display-message -p -t rf "off-after-output off=#{scroll_position} [#{copy_cursor_line}]"
# Turning it on twice is the same as once, and turning it off again stops it.
$TM send-keys -X -t rf refresh-on
$TM send-keys -X -t rf refresh-on
sleep 1
$TM send-keys -X -t rf refresh-off
$TM send-keys -X -t rf refresh-off
$TM display-message -p -t rf "on-then-off in=#{pane_in_mode} y=#{copy_cursor_y}"
# The toggle flips the same flag; both edges are exercised.
$TM send-keys -X -t rf refresh-toggle
sleep 1
$TM send-keys -X -t rf refresh-toggle
$TM display-message -p -t rf "toggled in=#{pane_in_mode} y=#{copy_cursor_y}"
# None of it disturbs a selection or the search state.
$TM send-keys -X -t rf history-top
$TM send-keys -X -t rf begin-selection
$TM send-keys -X -t rf cursor-right
$TM send-keys -X -t rf refresh-toggle
$TM send-keys -X -t rf refresh-toggle
$TM display-message -p -t rf "sel present=#{selection_present} s=#{selection_start_y},#{selection_start_x} e=#{selection_end_y},#{selection_end_x}"
$TM send-keys -X -t rf cancel
$TM display-message -p -t rf "after-cancel in=#{pane_in_mode}"
