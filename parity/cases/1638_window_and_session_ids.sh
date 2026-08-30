# Object ids are per-server counters: a fresh server hands out $0/@0/%0 and the
# next session id is one past the highest allocated.
$TM display-message -p 'session=#{session_id} window=#{window_id} pane=#{pane_id} next=#{next_session_id}'
$TM new-window -d
$TM new-session -d -s second
$TM display-message -p 'next=#{next_session_id}'
$TM list-windows -a -F '#{session_name}:#{window_id}' | sort
