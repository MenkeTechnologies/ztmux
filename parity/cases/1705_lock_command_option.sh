# lock-command is the server option the lock commands run; it round-trips and
# lock-server without a client is quiet about doing nothing.
$TM show -sv lock-command
$TM set -s lock-command 'true'
$TM show -sv lock-command
$TM lock-server 2>&1; echo "rc=$?"
$TM set -su lock-command
$TM show -sv lock-command
