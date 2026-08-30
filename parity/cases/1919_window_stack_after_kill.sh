# The session's window stack survives a kill: the window that was current before
# the killed one becomes current, and #{window_stack_index} renumbers around the
# hole.
$TM set -g automatic-rename off
$TM new-window -d -n one
$TM new-window -d -n two
$TM new-window -d -n three
$TM select-window -t one
$TM select-window -t two
$TM select-window -t three
echo "stack now:"; $TM list-windows -F '  #{window_name} stack=#{window_stack_index}' | sort
$TM kill-window -t three
echo "after killing the current window:"
echo "  current: $($TM display-message -p '#{window_name}')"
$TM list-windows -F '  #{window_name} stack=#{window_stack_index}' | sort
$TM kill-window -t one
echo "after killing one further down the stack:"
$TM list-windows -F '  #{window_name} stack=#{window_stack_index}' | sort
