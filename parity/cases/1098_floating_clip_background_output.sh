# A tiled pane writing output must not draw over a floating pane stacked above
# it. The float is created over pane 0, then pane 0 is made to emit a screenful
# of text and clear itself; capture-pane on the FLOAT shows whether anything
# leaked into it, and pane 0's own grid must still hold its own text.
$TM new-pane -d -x30 -y6 "cat"
$TM send-keys -t0 'printf "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX\\n%.0s" $(seq 1 20)' Enter
$TM send-keys -t0 'clear' Enter
$TM send-keys -t0 'printf "YYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYY\\n%.0s" $(seq 1 20)' Enter
# The float never received input, so its grid must be empty.
$TM capture-pane -t1 -p | tr -d ' \n' | head -c 40
echo "|float-content-above"
# The tiled pane keeps its own content.
$TM capture-pane -t0 -p | tr -d ' \n' | grep -c Y
$TM display-message -p 'geom #{pane_floating_flag} @#{pane_x},#{pane_y}'
