# move-window -s takes a window out of one session and puts it in another, where
# link-window would leave it in both.
$TM set -g automatic-rename off
$TM new-session -d -s dest -x 80 -y 24
$TM new-window -d -n travelling
echo "before:"
$TM list-windows -F '  0 #{window_index}:#{window_name}' | sort
$TM list-windows -t dest -F '  dest #{window_index}:#{window_name}' | sort
$TM move-window -s travelling -t dest:5; echo "rc=$?"
echo "after:"
$TM list-windows -F '  0 #{window_index}:#{window_name}' | sort
$TM list-windows -t dest -F '  dest #{window_index}:#{window_name}' | sort
echo "== moving onto an occupied index needs -k =="
$TM new-window -d -n other
$TM move-window -s other -t dest:5 2>&1; echo "rc=$?"
$TM move-window -k -s other -t dest:5; echo "with -k rc=$?"
$TM list-windows -t dest -F '  dest #{window_index}:#{window_name}' | sort
