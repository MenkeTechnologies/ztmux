# Session and window names are checked with check_name (tmux.c:300), which only
# rejects invalid UTF-8, so a name with a dot or a colon is accepted even though
# it collides with the target syntax. An empty name clears it back to the
# automatic one for a window and is refused for a session.
$TM set -g automatic-rename off
$TM rename-window 'has.dot'
$TM display-message -p 'window=[#{window_name}]'
$TM rename-window 'has:colon'
$TM display-message -p 'window=[#{window_name}]'
$TM rename-window ''; echo "empty window rc=$?"
$TM display-message -p 'window=[#{window_name}]'
$TM rename-session 'sess.dot'; echo "rc=$?"
$TM display-message -p 'session=[#{session_name}]'
$TM rename-session 'sess:colon'; echo "rc=$?"
$TM display-message -p 'session=[#{session_name}]'
$TM rename-session ''; echo "empty session rc=$?"
$TM display-message -p 'session=[#{session_name}]'
$TM rename-session '0'
