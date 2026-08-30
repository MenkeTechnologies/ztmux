# unbind removes one key; -a empties a whole table; unbinding a key that is not
# bound is an error unless -q is given.
$TM bind -T unbtest a set -g @a 1
$TM bind -T unbtest b set -g @b 1
$TM unbind -T unbtest a; echo "rc=$?"
$TM list-keys -T unbtest
$TM unbind -T unbtest a 2>&1; echo "missing rc=$?"
$TM unbind -q -T unbtest a 2>&1; echo "quiet rc=$?"
$TM unbind -a -T unbtest; echo "unbind -a rc=$?"
$TM list-keys -T unbtest 2>&1; echo "list rc=$?"
