# select-window's navigation flags: -n and -p step, -l goes to the last window,
# and -T toggles between the current one and the last.
$TM set -g automatic-rename off
$TM new-window -d -n one
$TM new-window -d -n two
$TM new-window -d -n three
$TM select-window -t one
$TM select-window -t three
echo "current: $($TM display-message -p '#{window_name}') last: $($TM display-message -p -t '{last}' '#{window_name}')"
$TM select-window -l; echo "-l -> $($TM display-message -p '#{window_name}')"
$TM select-window -n; echo "-n -> $($TM display-message -p '#{window_name}')"
$TM select-window -p; echo "-p -> $($TM display-message -p '#{window_name}')"
$TM select-window -T; echo "-T -> $($TM display-message -p '#{window_name}')"
$TM select-window -T; echo "-T again -> $($TM display-message -p '#{window_name}')"
echo "== -T on a window with no last is a no-op, not an error =="
$TM new-session -d -s alone -x 80 -y 24
$TM select-window -T -t alone 2>&1; echo "rc=$?"
