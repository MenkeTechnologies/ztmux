# list-windows -O sort / -r reverse, single-session and -a global sort.
$TM new-window -n wb "sleep 300"
$TM new-window -n wa "sleep 300"
$TM list-windows -O name -F '#{window_name}'
$TM list-windows -O name -r -F '#{window_name}'
$TM list-windows -a -O index -F '#{session_name}:#{window_index}'
$TM list-windows -O bogus
