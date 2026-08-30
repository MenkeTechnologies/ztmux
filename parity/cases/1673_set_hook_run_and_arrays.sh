# set-hook -R runs the hook immediately; hooks are arrays, so two indexes both
# fire and show-hooks lists them (cmd-set-hook.c:65).
$TM set-hook -g 'alert-bell[0]' 'set -g @bell0 fired'
$TM set-hook -g 'alert-bell[1]' 'set -g @bell1 fired'
$TM set -g @bell0 no
$TM set -g @bell1 no
$TM show-hooks -g | grep '^alert-bell' | sort
$TM set-hook -R alert-bell; echo "run rc=$?"
echo "bell0=$($TM show -gv @bell0) bell1=$($TM show -gv @bell1)"
$TM set-hook -gu 'alert-bell[0]'
$TM set-hook -gu 'alert-bell[1]'
$TM show-hooks -g | grep -c '^alert-bell'
