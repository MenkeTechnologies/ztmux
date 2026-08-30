# kill-window -a kills every window in the session but the target.
$TM set -g automatic-rename off
$TM new-window -d -n keep
$TM new-window -d -n gone1
$TM new-window -d -n gone2
$TM list-windows -F '#{window_name}' | sort | tr '\n' ' '; echo
$TM kill-window -a -t keep; echo "rc=$?"
$TM list-windows -F '#{window_name}' | sort
