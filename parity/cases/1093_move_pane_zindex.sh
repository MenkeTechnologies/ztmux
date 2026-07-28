# move-pane -z / -P front|back|forward|backward: restack floating panes.
# pane_z is the index among floating panes, front (0) first.
$TM new-pane -x20 -y5 "sleep 300"
$TM new-pane -x20 -y5 "sleep 300"
$TM new-pane -x20 -y5 "sleep 300"
order() { $TM list-panes -F '#{pane_index}:#{pane_floating_flag}:#{pane_z}' | tr '\n' ' '; echo "$1"; }
order start
$TM move-pane -P back
order back
$TM move-pane -P front
order front
$TM move-pane -P backward
order backward
$TM move-pane -P forward
order forward
$TM move-pane -P backward-loop
order backward-loop
$TM move-pane -P forward-loop
order forward-loop
$TM move-pane -z 2
order z2
$TM move-pane -z 0
order z0
$TM move-pane -z bogus
