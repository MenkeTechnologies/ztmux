# Sessions in a group share their windows: a window created in one appears in
# the other, and killing it removes it from both. The current window is per
# session, so selecting in one does not move the other.
$TM set -g automatic-rename off
$TM new-session -d -s lead -n first -x 80 -y 24
$TM new-session -d -s follow -t lead -x 80 -y 24
echo "== both start with the same windows =="
$TM list-windows -t lead -F 'lead #{window_index}:#{window_name}' | sort
$TM list-windows -t follow -F 'follow #{window_index}:#{window_name}' | sort
$TM new-window -d -t lead -n shared
echo "== a window created in lead =="
$TM list-windows -t follow -F 'follow #{window_index}:#{window_name}' | sort
$TM select-window -t lead:shared
echo "== selecting in lead only moves lead =="
$TM display-message -p -t lead 'lead current=#{window_name}'
$TM display-message -p -t follow 'follow current=#{window_name}'
$TM kill-window -t lead:shared
echo "== and killing it removes it from both =="
$TM list-windows -t follow -F 'follow #{window_index}:#{window_name}' | sort
