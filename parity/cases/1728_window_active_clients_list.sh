# The window's client formats are empty with nothing attached, and the session
# ones agree with them.
$TM list-windows -F '#{window_name} clients=#{window_active_clients} [#{window_active_clients_list}] sessions=#{window_active_sessions} [#{window_active_sessions_list}]'
$TM display-message -p 'session attached=#{session_attached} [#{session_attached_list}] many=#{session_many_attached}'
