# join-pane -p takes a percentage of the destination pane, the same argument
# split-window -p takes, and -l takes a number of lines instead.
$TM set -g automatic-rename off
$TM set -g status off
$TM new-window -d -n src 'sleep 300'
$TM new-window -d -n dst 'sleep 300'
echo "dst height: $($TM display-message -p -t dst.0 '#{pane_height}')"
$TM join-pane -d -v -p 25 -s src.0 -t dst.0; echo "join -p 25 rc=$?"
$TM list-panes -t dst -F '  pane #{pane_index} height #{pane_height}'
echo "== -l with a plain number of lines =="
$TM new-window -d -n src2 'sleep 300'
$TM join-pane -d -v -l 5 -s src2.0 -t dst.0; echo "join -l 5 rc=$?"
$TM list-panes -t dst -F '  pane #{pane_index} height #{pane_height}'
echo "== -l as a percentage is spelled with a trailing % =="
$TM new-window -d -n src3 'sleep 300'
$TM join-pane -d -v -l 20% -s src3.0 -t dst.0; echo "join -l 20% rc=$?"
$TM list-panes -t dst -F '  pane #{pane_index} height #{pane_height}'
echo "== a percentage over 100 is an error =="
$TM new-window -d -n src4 'sleep 300'
$TM join-pane -d -v -p 250 -s src4.0 -t dst.0 2>&1; echo "rc=$?"
echo "windows: $($TM list-windows -F '#{window_name}' | tr '\n' ' ')"
