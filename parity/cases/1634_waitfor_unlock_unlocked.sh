# Unlocking a channel that was never locked is an error naming the channel
# (cmd_wait_for_unlock, cmd-wait-for.c:225-228).
$TM wait-for -U nolock; echo "rc=$?"
