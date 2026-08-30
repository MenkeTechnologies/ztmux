# link-window -k kills whatever is already at the destination index instead of
# failing, and -a/-b place the link after or before the target.
$TM set -g automatic-rename off
$TM new-session -d -s dest -x 80 -y 24
$TM new-window -d -n source
$TM new-window -d -t dest:3 -n occupied
$TM link-window -s source -t dest:3 2>&1; echo "clash rc=$?"
$TM link-window -k -s source -t dest:3; echo "-k rc=$?"
$TM list-windows -t dest -F '#{window_index}:#{window_name}' | sort
$TM unlink-window -t dest:3
$TM list-windows -t dest -F '#{window_index}:#{window_name}' | sort
