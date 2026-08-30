# With no client attached, list-clients prints nothing whatever the format, the
# filter or the order say, and only an unknown -O is an error (cmd-list-clients.c).
echo "clients: $($TM list-clients | wc -l | tr -d ' ')"
echo "[$($TM list-clients -F '#{client_name}')]"
echo "[$($TM list-clients -f '#{==:#{client_name},anything}' -F '#{client_name}')]"
echo "-O name: rc=$($TM list-clients -O name >/dev/null 2>&1; echo $?)"
echo "-O name -r: rc=$($TM list-clients -O name -r >/dev/null 2>&1; echo $?)"
echo "-O activity: rc=$($TM list-clients -O activity >/dev/null 2>&1; echo $?)"
$TM list-clients -O nosuchorder 2>&1; echo "unknown order rc=$?"
echo "== -t names a session that exists, and one that does not =="
$TM list-clients -t 0 2>&1; echo "rc=$?"
$TM list-clients -t nosuchsession 2>&1; echo "rc=$?"
