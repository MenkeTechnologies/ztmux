# break-pane and join-pane move a pane between layout trees, which means
# detaching its cell from one parent and splicing it into another at a computed
# offset. Both windows' full geometry has to be right afterwards, and the
# source window has to collapse correctly — the same recompute that killing a
# pane triggers, but with the cell surviving in a different tree.
$TM new-window -d -n src 'sleep 300'
$TM split-window -d -t src 'sleep 300'
$TM split-window -d -h -t src 'sleep 300'
$TM new-window -d -n dst 'sleep 300'
geo() { $TM list-panes -a -F "$1 #{window_name}.#{pane_index} #{pane_width}x#{pane_height} @#{pane_left},#{pane_top}"; }
geo start
$TM break-pane -d -s src.1 -n broke
geo after-break
$TM join-pane -d -s broke.0 -t dst.0
geo after-join
$TM join-pane -d -h -s dst.1 -t src.0
geo after-join-h
$TM list-windows -F '#{window_name} panes=#{window_panes} layout=#{window_layout}'
# Joining a pane to its own window, and to a missing target, are both errors.
$TM join-pane -d -s src.0 -t src.0 2>&1
$TM join-pane -d -s src.0 -t nosuchwindow 2>&1
$TM list-windows -F '#{window_name} panes=#{window_panes}'
