# The same check_name/clean_name pair guards the names given at creation time:
# new-session -s / -n and new-window -n (cmd-new-session.c:102-121,
# cmd-new-window.c:73-83). A control character is refused by check_name; the
# target separators are ordinary characters and survive.
$TM set -g automatic-rename off
$TM new-session -d -s 'made.dot' -x 80 -y 24; echo "rc=$?"
$TM new-session -d -s "$(printf 'bad\tname')" -x 80 -y 24 2>&1 | perl -pe 's/\t/<TAB>/'; echo "rc=${PIPESTATUS[0]}"
$TM new-window -d -n 'win.dot'; echo "rc=$?"
$TM new-window -d -n "$(printf 'bad\tname')" 2>&1 | perl -pe 's/\t/<TAB>/'; echo "rc=${PIPESTATUS[0]}"
$TM new-session -d -s ok -n "$(printf 'bad\tname')" -x 80 -y 24 2>&1 | perl -pe 's/\t/<TAB>/'; echo "rc=${PIPESTATUS[0]}"
echo "== what exists now =="
$TM list-sessions -F '#{session_name}' | sort
$TM list-windows -F '#{window_name}' | sort
