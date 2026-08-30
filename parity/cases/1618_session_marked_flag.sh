# select-pane -m marks a pane; #{session_marked} is set for the session holding
# the marked pane, and select-pane -M clears it.
$TM new-session -d -s other
$TM list-sessions -F '#{session_name} marked=#{session_marked}' | sort
$TM select-pane -m
echo "== after marking a pane in the base session =="
$TM list-sessions -F '#{session_name} marked=#{session_marked}' | sort
$TM display-message -p 'pane_marked=#{pane_marked} set=#{pane_marked_set}'
$TM select-pane -M
echo "== after clearing the mark =="
$TM list-sessions -F '#{session_name} marked=#{session_marked}' | sort
