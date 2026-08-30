# destroy-unattached=keep-last picks a DIFFERENT survivor inside a session group.
#
# With sessions named 0, alone, grouped1, grouped2 (the last three in one group)
# and no client attached:
#
#   next-3.7:  0 grouped1
#   ztmux   :  0 grouped2
#
# Both destroy two of the three grouped sessions and keep one, as keep-last
# says; they disagree about WHICH. The count and the rule agree -- rename the
# sessions and the two binaries pick the same survivor again (alone/base/g1/g2
# both keep g2), so this is not a difference in the keep-last test itself.
#
# The cause is the traversal. server_check_unattached walks the session tree
# with RB_FOREACH and calls session_destroy inside the loop (server-fn.c:487-507);
# RB_FOREACH computes RB_NEXT *after* the body, so the C reads the links of a
# node that RB_REMOVE has already taken out of the tree and rebalanced around.
# Where the walk lands next therefore depends on the tree's shape, which is why
# only some sets of names show it. This port's rb_foreach takes the next pointer
# before handing the node to the body, so it walks the remaining tree instead.
#
# Matching the C here would mean reading a session's tree links after it has
# been destroyed -- a use-after-free in this port rather than the C's
# happens-to-work read. That is why this is recorded rather than half-fixed:
# closing it needs the traversal reworked so the C's order falls out without the
# dangling read. Everything else about destroy-unattached agrees and is compared
# by parity/cases/1955_destroy_unattached_choices.sh.
$TM new-session -d -s alone 'sleep 300'
$TM new-session -d -t alone -s grouped1
$TM new-session -d -t alone -s grouped2
echo "before: $($TM list-sessions -F '#{session_name}' | tr '\n' ' ')"
$TM set -g destroy-unattached keep-last
sleep 0.5
echo "after:  $($TM list-sessions -F '#{session_name}' 2>&1 | tr '\n' ' ')"
