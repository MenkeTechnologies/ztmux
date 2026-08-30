# find-window searches window names, titles and contents and opens the result in
# a tree mode, so it needs a client; its flags and its arity are checked first.
$TM find-window 2>&1; echo "no argument rc=$?"
$TM find-window pattern 2>&1; echo "with a pattern rc=$?"
$TM find-window -C -N -T pattern 2>&1; echo "-CNT rc=$?"
$TM find-window -r pattern 2>&1; echo "-r rc=$?"
$TM find-window -Q pattern 2>&1; echo "bad flag rc=$?"
$TM list-commands find-window
