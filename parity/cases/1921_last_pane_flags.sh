# last-pane takes the same -d, -e and -Z flags select-pane does: -d and -e turn
# the pane's input off and on, and -Z keeps a zoom across the move.
$TM split-window -d
$TM select-pane -t 0
$TM select-pane -t 1
$TM last-pane; echo "bare rc=$?"
echo "active: $($TM display-message -p '#{pane_index}')"
$TM last-pane -d; echo "-d rc=$?"
$TM list-panes -F '  #{pane_index} input_off=#{pane_input_off} active=#{pane_active}' | sort
$TM last-pane -e; echo "-e rc=$?"
$TM list-panes -F '  #{pane_index} input_off=#{pane_input_off} active=#{pane_active}' | sort
$TM resize-pane -Z
$TM last-pane -Z; echo "-Z rc=$?"
echo "zoomed=$($TM display-message -p '#{window_zoomed_flag}') active=$($TM display-message -p '#{pane_index}')"
