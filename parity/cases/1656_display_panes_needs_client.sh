# display-panes and the other client-only commands report that they have no
# client rather than doing nothing, when run from a command line with no client
# attached to the target session.
$TM display-panes -d 1 2>&1; echo "rc=$?"
$TM refresh-client 2>&1; echo "rc=$?"
$TM choose-tree 2>&1; echo "rc=$?"
$TM clock-mode 2>&1; echo "rc=$?"
