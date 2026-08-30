# link-window -a and -b put the link after or before the target index, and
# unlink-window -k kills the window if that link was its last.
$TM set -g automatic-rename off
$TM new-session -d -s dest -n anchor -x 80 -y 24
$TM new-window -d -n source
$TM link-window -a -s source -t dest:anchor; echo "-a rc=$?"
$TM list-windows -t dest -F '  #{window_index}:#{window_name}' | sort
$TM link-window -b -s source -t dest:anchor; echo "-b rc=$?"
$TM list-windows -t dest -F '  #{window_index}:#{window_name}' | sort
echo "== unlink without -k leaves it where it still exists =="
$TM unlink-window -t dest:1
$TM list-windows -t dest -F '  #{window_index}:#{window_name}' | sort
echo "== -k on the last link kills the window =="
$TM new-window -d -n solo
$TM unlink-window -k -t solo 2>&1; echo "rc=$?"
$TM list-windows -F '  #{window_index}:#{window_name}' | sort
