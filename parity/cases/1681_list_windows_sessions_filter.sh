# -f filters the listing with a format; a filter matching nothing prints nothing
# and still succeeds.
$TM set -g automatic-rename off
$TM new-window -d -n keep
$TM new-window -d -n drop
$TM list-windows -f '#{m:keep*,#{window_name}}' -F '#{window_name}'; echo "rc=$?"
$TM list-windows -f '#{==:#{window_panes},1}' -F '#{window_name}' | sort
$TM list-windows -f '#{==:1,0}' -F '#{window_name}'; echo "empty rc=$?"
$TM new-session -d -s other -x 80 -y 24
$TM list-sessions -f '#{==:#{session_name},other}' -F '#{session_name}'; echo "rc=$?"
