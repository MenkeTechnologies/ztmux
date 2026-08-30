# attach-session from a command line with no terminal fails the same way for
# each of its flags; an unknown session is a target error before that.
$TM attach-session -t 0 2>&1; echo "rc=$?"
$TM attach-session -E -t 0 2>&1; echo "rc=$?"
$TM attach-session -r -t 0 2>&1; echo "rc=$?"
$TM attach-session -t nosuchsession 2>&1; echo "rc=$?"
$TM attach-session -c /tmp -t 0 2>&1; echo "rc=$?"
