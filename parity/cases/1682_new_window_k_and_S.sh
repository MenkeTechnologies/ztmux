# -k kills an existing window at the target index instead of failing; -S selects
# the existing one and creates nothing.
$TM set -g automatic-rename off
$TM new-window -d -t 5 -n first
$TM new-window -d -t 5 -n second 2>&1; echo "clash rc=$?"
$TM new-window -d -k -t 5 -n replaced; echo "-k rc=$?"
$TM list-windows -F '#{window_index}:#{window_name}' | sort
$TM new-window -d -S -t 5 -n replaced; echo "-S rc=$?"
$TM list-windows -F '#{window_index}:#{window_name}' | sort
