# Locking an unlocked channel returns at once (cmd-wait-for.c:213-215); the
# matching unlock with no queued lockers clears and removes the channel, so a
# second unlock is back to "not locked".
$TM wait-for -L lk; echo "lock rc=$?"
$TM wait-for -U lk; echo "unlock rc=$?"
$TM wait-for -U lk; echo "unlock again rc=$?"
