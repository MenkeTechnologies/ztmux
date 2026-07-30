# capture-pane reads the PANE's grid, not the screen copy mode is painting over
# it: without -M the C explicitly takes &wp->base, so entering copy mode and
# scrolling back must not change a single byte of what capture returns. That is
# the same confusion that made #{copy_cursor_line} report copy mode's own screen
# (with the position indicator baked into it) instead of the backing grid.
$TM new-window -d -n cmcap 'i=1; while [ $i -le 30 ]; do echo "row-$i"; i=$((i+1)); done; sleep 300'
sleep 1
echo "== before"
$TM capture-pane -p -S 0 -E 3 -t cmcap | perl -pe "s{^(.*)\$}{[\$1]}"
$TM copy-mode -t cmcap
$TM send-keys -X -t cmcap history-top
echo "== in copy mode, scrolled to the top"
$TM capture-pane -p -S 0 -E 3 -t cmcap | perl -pe "s{^(.*)\$}{[\$1]}"
$TM capture-pane -pe -S -3 -E -1 -t cmcap | perl -pe 's/\e/<ESC>/g' | perl -pe "s{^(.*)\$}{[\$1]}"
$TM display-message -p -t cmcap 'mode=#{pane_mode} off=#{scroll_position}'
# A selection in flight must not leak into the capture either.
$TM send-keys -X -t cmcap begin-selection
$TM send-keys -X -t cmcap cursor-down
$TM send-keys -X -t cmcap cursor-right
echo "== with a selection"
$TM capture-pane -p -S 0 -E 3 -t cmcap | perl -pe "s{^(.*)\$}{[\$1]}"
$TM send-keys -X -t cmcap cancel
echo "== after cancel"
$TM capture-pane -p -S 0 -E 3 -t cmcap | perl -pe "s{^(.*)\$}{[\$1]}"
$TM display-message -p -t cmcap 'mode=[#{pane_mode}] in=#{pane_in_mode}'
