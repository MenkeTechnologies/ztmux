# display-popup argument parsing: -k (any key dismisses) and -N (reset flags
# when modifying an open popup) must be accepted, and unknown flags rejected
# with the same message. No client is attached here, so every accepted form
# stops at the same "no current client" error, which is exactly what makes the
# accept/reject split observable without a tty.
$TM display-popup -Z 2>&1
$TM display-popup -k 2>&1
$TM display-popup -N 2>&1
$TM display-popup -kNE true 2>&1
$TM display-popup -EE -k -T title 2>&1
# The usage string is printed for a malformed invocation.
$TM display-popup -b 2>&1
