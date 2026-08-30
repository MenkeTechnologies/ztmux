# switch-client needs a client for -n/-p/-l, and says so rather than doing
# nothing; -t with an unknown session is a target error either way.
$TM new-session -d -s other -x 80 -y 24
$TM switch-client -n 2>&1; echo "rc=$?"
$TM switch-client -p 2>&1; echo "rc=$?"
$TM switch-client -l 2>&1; echo "rc=$?"
$TM switch-client -t other 2>&1; echo "rc=$?"
$TM switch-client -t nosuchsession 2>&1; echo "rc=$?"
