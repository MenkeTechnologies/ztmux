# The selection format variables are the only observable window onto copy mode's
# selection state without a client. begin-selection anchors it, every motion
# extends it, other-end swaps which end the cursor is on, stop-selection freezes
# it and clear-selection drops it — each of those writes a different pair of
# coordinates, and a port that stores the anchor in screen coordinates instead
# of grid coordinates reports the right shape with the wrong numbers.
$TM new-window -d -n sel 'printf "alpha bravo charlie\ndelta echo foxtrot\ngolf hotel india\n"; sleep 300'
sleep 1
$TM copy-mode -t sel
$TM send-keys -X -t sel history-top
$TM send-keys -X -t sel start-of-line
show() {
  $TM display-message -p -t sel \
    "$1 cur=#{copy_cursor_y},#{copy_cursor_x} present=#{selection_present} active=#{selection_active} mode=#{selection_mode} s=#{selection_start_y},#{selection_start_x} e=#{selection_end_y},#{selection_end_x}"
}
show none
$TM send-keys -X -t sel begin-selection; show begin
$TM send-keys -X -t sel cursor-right; $TM send-keys -X -t sel cursor-right; show right2
$TM send-keys -X -t sel cursor-down; show down
$TM send-keys -X -t sel other-end; show other-end
$TM send-keys -X -t sel cursor-right; show after-swap
$TM send-keys -X -t sel stop-selection; show stop
$TM send-keys -X -t sel cursor-down; show moved-after-stop
$TM send-keys -X -t sel clear-selection; show clear
