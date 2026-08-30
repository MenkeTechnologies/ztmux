# #{window_stack_index} is the window's position in the session's most-recently
# used stack (1 = current); #{active_window_index} and #{last_window_index} track
# the session's current and highest window index.
$TM new-window -d -n w1
$TM new-window -d -n w2
$TM select-window -t w1
$TM select-window -t w2
$TM list-windows -F '#{window_name} stack=#{window_stack_index}' | sort
$TM display-message -p 'active=#{active_window_index} last=#{last_window_index}'
