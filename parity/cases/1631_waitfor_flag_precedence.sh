# The exec tests S before L before U (cmd-wait-for.c:131-136), so `-SL` signals
# and never locks: the proof is that the following unlock says "not locked".
$TM wait-for -SL both; echo "rc=$?"
$TM wait-for -U both; echo "rc=$?"
# ...and the signal did happen: this wait returns rather than blocking.
$TM wait-for both; echo "wait rc=$?"
