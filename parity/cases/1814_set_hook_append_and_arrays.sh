# set-hook -a appends another command to a hook rather than replacing it, so
# both run; -u removes the whole hook and show-hooks lists what is left.
$TM set -g @log ''
$TM set-hook -g after-new-window 'set -ga @log ",first"'
$TM set-hook -ga after-new-window 'set -ga @log ",second"'
$TM show-hooks -g | grep '^after-new-window' | sort
$TM new-window -d
echo "log=[$($TM show -gv @log)]"
$TM set-hook -gu after-new-window
echo "after -u: [$($TM show-hooks -g | grep -c '^after-new-window')]"
echo "== the same with an indexed hook =="
$TM set -g @log ''
$TM set-hook -g 'after-new-window[3]' 'set -ga @log ",indexed"'
$TM new-window -d
echo "log=[$($TM show -gv @log)]"
$TM set-hook -gu 'after-new-window[3]'
