# break-pane -n names the new window, -P -F prints it, -a puts it after the
# current window and -d leaves it unselected.
$TM set -g automatic-rename off
$TM split-window -d
$TM break-pane -d -n broken -P -F '#{window_index}:#{window_name}'; echo "rc=$?"
$TM list-windows -F '#{window_index}:#{window_name}' | sort
echo "== -a places it after the current window =="
$TM split-window -d
$TM break-pane -d -a -n after-current -P -F '#{window_index}'
$TM list-windows -F '#{window_index}:#{window_name}' | sort
echo "== breaking the only pane of a window =="
$TM break-pane -d -t broken 2>&1; echo "rc=$?"
