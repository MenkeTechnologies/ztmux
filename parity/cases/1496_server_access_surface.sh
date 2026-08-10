# The server-access command surface: the listing format, the -g group flag and
# every rejection path.
#
# What this case CANNOT reach: the read-only bit itself. Both binaries refuse to
# change the access of the user who owns the server ("owns the server, can't
# change access"), and the parity harness has exactly one user, so -r/-w never
# get as far as setting or clearing SERVER_ACL_READONLY. The inverted deny_write
# this suite failed to catch is only observable with a second real user attached.
# The listing's flag column is still worth pinning, because it is what any such
# change would show up in.
#
# The owning user's name is replaced so the case is host-independent; the (U,W)
# flag column, which is the part under test, is left alone.
norm() { perl -pe 's/^\S+/USER/'; }
$TM server-access -l | norm
U=$($TM server-access -l | awk '{print $1; exit}')
# Refused: the server owner cannot change their own access, either way round.
$TM server-access -r "$U" 2>&1 | norm
$TM server-access -w "$U" 2>&1 | norm
$TM server-access -l | norm
# Rejection paths: unknown user, unknown group, and a missing operand.
$TM server-access -a nosuchuser1 2>&1
$TM server-access -d nosuchuser1 2>&1
$TM server-access -g nosuchgroup1 2>&1
$TM server-access -ag nosuchgroup1 2>&1
$TM server-access -a 2>&1
$TM server-access -x 2>&1
# The list must be unchanged by everything above.
$TM server-access -l | norm
