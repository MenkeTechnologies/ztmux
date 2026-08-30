# after-lock-server and after-display-panes fire even when the command they
# follow could not do anything for want of a client, because the hook runs off
# the command's completion rather than its effect.
$TM set -g @log ''
$TM set-hook -g after-lock-server 'set -ga @log ",lock"'
$TM set-hook -g after-display-panes 'set -ga @log ",panes"'
$TM lock-server 2>&1; echo "lock-server rc=$?"
$TM display-panes -d 1 2>&1; echo "display-panes rc=$?"
echo "log=[$($TM show -gv @log)]"
$TM set-hook -gu after-lock-server
$TM set-hook -gu after-display-panes
