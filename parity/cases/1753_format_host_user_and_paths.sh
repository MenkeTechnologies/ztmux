# #{uid}, #{host_short} and the path formats describe the machine and the
# process rather than the session; both binaries run as the same user on the
# same host from the same directory, so they compare -- what is NOT comparable
# is anything naming the binary or its pid (socket_path, pane_pid, pane_tty,
# client_pid), which is why no case pins those.
$TM display-message -p 'uid matches id -u: #{==:#{uid},'"$(id -u)"'}'
$TM display-message -p 'host_short is the host without its domain: #{==:#{host_short},'"$(hostname -s)"'}'
$TM display-message -p 'pane_path is empty until a program sets it: [#{pane_path}]'
$TM display-message -p 'session_path is the start directory: #{==:#{session_path},#{pane_start_path}}'
$TM display-message -p 'session_active: #{session_active}'
