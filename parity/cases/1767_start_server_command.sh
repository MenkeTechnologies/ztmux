# start-server takes no arguments and is idempotent: the server is already
# running here, so it does nothing and says nothing. It also carries
# CMD_STARTSERVER, which is what makes it usable as `tmux start-server` on a
# machine with no server at all.
$TM start-server; echo "rc=$?"
$TM start-server; echo "again rc=$?"
$TM list-sessions -F '#{session_name}' | sort
$TM start-server extra-argument 2>&1; echo "rc=$?"
$TM list-commands start-server
