# swap-window can name windows in different sessions: the two exchange places,
# each session keeping the same number of windows.
$TM set -g automatic-rename off
$TM new-session -d -s other -x 80 -y 24
$TM new-window -d -n mine
$TM new-window -d -t other -n theirs
echo "before:"
$TM list-windows -F '  0 #{window_index}:#{window_name}' | sort
$TM list-windows -t other -F '  other #{window_index}:#{window_name}' | sort
$TM swap-window -s mine -t other:theirs; echo "rc=$?"
echo "after:"
$TM list-windows -F '  0 #{window_index}:#{window_name}' | sort
$TM list-windows -t other -F '  other #{window_index}:#{window_name}' | sort
