# Signalling a channel with no waiters marks it woken (cmd-wait-for.c:148-151),
# so the next wait returns immediately instead of blocking.
$TM wait-for -S chan; echo "signal rc=$?"
$TM wait-for chan; echo "wait rc=$?"
