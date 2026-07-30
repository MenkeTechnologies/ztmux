# capture-pane -M captures what the pane's MODE is showing rather than the
# pane's own grid: in copy mode that is the mode's backing screen, which the
# mode exposes through its get_screen callback. Without -M the same command
# must still read the pane's base screen, so the two differ exactly when a mode
# is open and are identical when none is.
$TM new-window -d -n ms 'i=1; while [ $i -le 30 ]; do echo "row-$i"; i=$((i+1)); done; sleep 300'
sleep 1
echo "== no mode: -M and plain agree"
$TM capture-pane -p -S 0 -E 2 -t ms | perl -pe "s{^(.*)\$}{[\$1]}"
$TM capture-pane -pM -S 0 -E 2 -t ms | perl -pe "s{^(.*)\$}{[\$1]}"
$TM copy-mode -t ms
$TM send-keys -X -t ms history-top
echo "== in copy mode, plain"
$TM capture-pane -p -S 0 -E 2 -t ms | perl -pe "s{^(.*)\$}{[\$1]}"
echo "== in copy mode, -M"
$TM capture-pane -pM -S 0 -E 2 -t ms | perl -pe "s{^(.*)\$}{[\$1]}"
echo "== -M with -L and -F"
$TM capture-pane -pMLF -S 0 -E 2 -t ms | perl -pe "s{^(.*)\$}{[\$1]}"
echo "== -M after leaving the mode"
$TM send-keys -X -t ms cancel
$TM capture-pane -pM -S 0 -E 2 -t ms | perl -pe "s{^(.*)\$}{[\$1]}"
$TM display-message -p -t ms 'mode=[#{pane_mode}] in=#{pane_in_mode}'
