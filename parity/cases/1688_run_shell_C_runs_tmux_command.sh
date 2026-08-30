# -C runs the argument as a tmux command instead of a shell command
# (cmd-run-shell.c:47), so no shell is involved and tmux errors come back.
$TM run-shell -C 'set -g @via_C yes'; echo "rc=$?"
echo "value=$($TM show -gv @via_C)"
$TM run-shell -C 'display-message -p from-C'; echo "rc=$?"
echo "== an unknown tmux command =="
$TM run-shell -C 'nosuchcommand' 2>&1; echo "rc=$?"
