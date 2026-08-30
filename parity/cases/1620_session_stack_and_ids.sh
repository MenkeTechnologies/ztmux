# #{session_stack} is the client's session stack; with no attached client it is
# just the session itself, and #{server_sessions} counts every session.
$TM new-session -d -s alpha
$TM new-session -d -s beta
$TM list-sessions -F '#{session_name} stack=#{session_stack} sessions=#{server_sessions}' | sort
