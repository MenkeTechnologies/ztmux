# command-error fires when a command in a queue fails, and #{hook} names it.
$TM set -g @err ''
$TM set-hook -g command-error 'set -gF @err "#{hook}"'
$TM kill-window -t nosuchwindow 2>&1; echo "rc=$?"
echo "hook saw: [$($TM show -gv @err)]"
$TM set -g @err ''
$TM display-message -p ok >/dev/null
echo "after a command that succeeds: [$($TM show -gv @err)]"
$TM set-hook -gu command-error
