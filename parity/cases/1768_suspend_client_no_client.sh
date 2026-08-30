# suspend-client stops the client process with SIGTSTP; from a command line with
# no client there is nothing to suspend and it says so, and an unknown -t is a
# target error.
$TM suspend-client 2>&1; echo "rc=$?"
$TM suspend-client -t /dev/nosuchtty 2>&1; echo "rc=$?"
$TM list-commands suspend-client
