# destroy-unattached, the one choice option that cannot be swept with the rest
# (case 1954): with no client attached, setting it to anything but "off" takes
# the session away, and with the last session the server goes too. The names are
# pinned here with what each one does.
#
# keep-last's choice of survivor inside a group of three is left to
# parity/known_gaps/destroy_unattached_keep_last_survivor.sh: the two binaries
# agree on the rule and the count but not always on which session survives,
# because the C's traversal reads a node it has already removed.
$TM set -g status off
echo "default: [$($TM show -gv destroy-unattached)]"
echo "== off leaves the unattached sessions alone =="
$TM set -g destroy-unattached off; echo "rc=$?"
$TM new-session -d -s alone 'sleep 300'
sleep 0.4
echo "sessions: $($TM list-sessions -F '#{session_name}' | tr '\n' ' ')"
echo "== keep-last keeps a session that is alone in its group =="
$TM set -g destroy-unattached keep-last; echo "rc=$?"
sleep 0.5
echo "sessions: $($TM list-sessions -F '#{session_name}' 2>&1 | tr '\n' ' ')"
echo "== with two in a group, keep-last leaves exactly one of them =="
$TM set -g destroy-unattached off
$TM new-session -d -t alone -s grouped1
echo "group size: $($TM list-sessions -F '#{session_group_size}' -f '#{==:#{session_name},alone}')"
$TM set -g destroy-unattached keep-last; echo "rc=$?"
sleep 0.5
echo "sessions left: $($TM list-sessions | wc -l | tr -d ' ')"
echo "one of the group survives: $($TM list-sessions -F '#{session_name}' | grep -c -E '^(alone|grouped1)$')"
echo "== keep-group keeps a whole group and takes the ungrouped ones =="
$TM set -g destroy-unattached keep-group; echo "rc=$?"
sleep 0.5
echo "sessions left: $($TM list-sessions | wc -l | tr -d ' ')"
echo "the survivor is in a group: $($TM list-sessions -F '#{session_grouped}')"
echo "== on takes every unattached session, and the server with them =="
$TM set -g destroy-unattached on; echo "rc=$?"
sleep 0.5
echo "sessions: [$($TM list-sessions 2>&1 | head -1 | grep -c 'no server running')]"
