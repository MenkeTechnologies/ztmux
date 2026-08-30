# Objects can be addressed by id ($0, @0, %0) or by the session:window.pane
# string form, and the two must agree.
$TM set -g automatic-rename off
$TM new-window -d -n named
$TM split-window -d -t named
sid=$($TM display-message -p '#{session_id}')
wid=$($TM display-message -p -t named '#{window_id}')
pid=$($TM display-message -p -t named.1 '#{pane_id}')
echo "by id:     $($TM display-message -p -t "$pid" '#{session_name}:#{window_name}.#{pane_index}')"
echo "by string: $($TM display-message -p -t '0:named.1' '#{session_name}:#{window_name}.#{pane_index}')"
echo "window id: $($TM display-message -p -t "$wid" '#{window_name}')"
echo "session:   $($TM display-message -p -t "$sid" '#{session_name}')"
echo "== a window id that does not exist =="
$TM display-message -p -t '@999' '#{window_name}' 2>&1; echo "rc=$?"
