# after-set-hook fires for the command that arms hooks, and after-queue fires
# when a command queue drains -- so arming after-queue makes it fire for its own
# queue too.
$TM set -g @log ''
$TM set-hook -g after-set-hook 'set -ga @log ",set-hook"'
$TM set-hook -g @dummy 'display-message x' 2>&1; echo "arming a bogus hook rc=$?"
$TM set-hook -g alert-bell 'display-message x'
echo "after-set-hook fired: [$($TM show -gv @log)]"
$TM set -g @log ''
$TM set-hook -g after-queue 'set -ga @log ",queue"'
$TM display-message -p 'one command'
echo "after-queue fired at least once: $($TM show -gv @log | grep -c queue)"
$TM set-hook -gu after-queue
$TM set-hook -gu after-set-hook
$TM set-hook -gu alert-bell
