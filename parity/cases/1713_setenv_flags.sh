# set-environment -h hides a variable from show-environment's plain listing, -r
# marks it to be removed from the environment of new processes, and -u unsets it.
$TM set-environment -t 0 PLAIN visible
$TM set-environment -h -t 0 HIDDEN secret
$TM set-environment -r -t 0 REMOVED
echo "== plain listing =="
$TM show-environment -t 0 | grep -E '^(PLAIN|HIDDEN|-REMOVED)' | sort
echo "== -h listing shows the hidden one =="
$TM show-environment -h -t 0 | grep -E '^(PLAIN|HIDDEN)' | sort
echo "== single lookups =="
$TM show-environment -t 0 PLAIN
$TM show-environment -t 0 HIDDEN 2>&1; echo "rc=$?"
$TM set-environment -u -t 0 PLAIN
$TM show-environment -t 0 PLAIN 2>&1; echo "rc=$?"
