# unbind -a on a table that was never created, and list-keys on the same: both
# say what they found rather than inventing an empty table.
$TM unbind -a -T ztpar-never-made 2>&1; echo "unbind -a rc=$?"
$TM list-keys -T ztpar-never-made 2>&1; echo "list-keys rc=$?"
$TM unbind -T ztpar-never-made x 2>&1; echo "unbind one rc=$?"
echo "== after creating and emptying it =="
$TM bind -T ztpar-made x display-message x
$TM list-keys -T ztpar-made -F '#{key}'
$TM unbind -a -T ztpar-made; echo "rc=$?"
$TM list-keys -T ztpar-made 2>&1; echo "rc=$?"
