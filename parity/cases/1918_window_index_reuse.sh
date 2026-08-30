# A killed window's index is free for reuse, and renumber-windows decides
# whether the gaps close by themselves when a window goes away.
$TM set -g automatic-rename off
$TM set -g renumber-windows off
$TM new-window -d -n a
$TM new-window -d -n b
$TM new-window -d -n c
$TM list-windows -F '  #{window_index}:#{window_name}' | sort
$TM kill-window -t b
echo "after killing the middle one, gaps stay:"
$TM list-windows -F '  #{window_index}:#{window_name}' | sort
$TM new-window -d -n d
echo "and a new window takes the free index:"
$TM list-windows -F '  #{window_index}:#{window_name}' | sort
$TM set -g renumber-windows on
$TM kill-window -t d
echo "with renumber-windows on, the gap closes:"
$TM list-windows -F '  #{window_index}:#{window_name}' | sort
$TM set -gu renumber-windows
