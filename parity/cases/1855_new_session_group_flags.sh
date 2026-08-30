# new-session -t joins the target's group and refuses the flags that would
# contradict it: a window name or a command cannot be given for a session that
# adopts another's windows.
$TM set -g automatic-rename off
$TM new-session -d -s lead -n named -x 80 -y 24
$TM new-session -d -s follower -t lead -x 80 -y 24; echo "plain -t rc=$?"
$TM list-sessions -F '#{session_name} group=[#{session_group}]' | sort
$TM list-windows -t follower -F 'follower #{window_index}:#{window_name}' | sort
echo "== -t with a window name =="
$TM new-session -d -s bad1 -t lead -n other -x 80 -y 24 2>&1; echo "rc=$?"
echo "== -t with a command =="
$TM new-session -d -s bad2 -t lead -x 80 -y 24 'sleep 300' 2>&1; echo "rc=$?"
echo "== the group is unchanged =="
$TM list-sessions -F '#{session_name}' | sort | tr '\n' ' '; echo
