# move-window -r renumbers every window in the session to close the gaps, and
# the base-index option decides where the numbering starts.
$TM set -g automatic-rename off
$TM new-window -d -t 4 -n four
$TM new-window -d -t 9 -n nine
$TM list-windows -F '#{window_index}:#{window_name}' | sort
$TM move-window -r; echo "rc=$?"
$TM list-windows -F '#{window_index}:#{window_name}' | sort
$TM set -g base-index 10
$TM move-window -r
$TM list-windows -F '#{window_index}:#{window_name}' | sort
