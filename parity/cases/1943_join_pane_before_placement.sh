# join-pane -b, the case that recorded a divergence until layout_get_tiled_cell
# was ported (layout.c:1593). next-3.7 routes join-pane through that wrapper and
# leaves cmd_join_pane_exec's own `flags` at zero (cmd-join-pane.c:379,419), so
# -b reaches the layout but never the pane-list insert; this port called
# layout_split_pane directly and put the joined pane in the other sub-cell.
$TM set -g automatic-rename off
$TM new-window -d -n src 'sleep 300'
$TM new-window -d -n src2 'sleep 300'
$TM new-window -d -n dst 'sleep 300'
$TM join-pane -h -s src -t dst.0
echo "after -h:"
$TM list-panes -t dst -F '  #{pane_index}@#{pane_left}+#{pane_width}'
$TM join-pane -b -h -s src2 -t dst.0
echo "after -b -h:"
$TM list-panes -t dst -F '  #{pane_index}@#{pane_left}+#{pane_width}'
