# wait-for takes exactly one channel argument (cmd-wait-for.c:37, args 1,1), so a
# bare `wait-for` is a usage error, not a hang.
$TM wait-for; echo "rc=$?"
$TM wait-for a b; echo "rc=$?"
