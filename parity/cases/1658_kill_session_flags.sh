# -a kills every session but the target; -C clears alerts on the target instead
# of killing it (cmd-kill-session.c:41).
$TM new-session -d -s keep -x 80 -y 24
$TM new-session -d -s gone1 -x 80 -y 24
$TM new-session -d -s gone2 -x 80 -y 24
$TM kill-session -C -t keep; echo "clear-alerts rc=$?"
$TM list-sessions -F '#{session_name}' | sort
$TM kill-session -a -t keep; echo "kill-others rc=$?"
$TM list-sessions -F '#{session_name}' | sort
