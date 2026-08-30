# -A attaches to an existing session instead of failing when the name is taken;
# without it a duplicate name is an error.
$TM new-session -d -s dup -x 80 -y 24; echo "first rc=$?"
$TM new-session -d -s dup -x 80 -y 24 2>&1; echo "duplicate rc=$?"
$TM new-session -A -d -s dup -x 80 -y 24 2>&1; echo "with -A rc=$?"
$TM list-sessions -F '#{session_name}' | sort
