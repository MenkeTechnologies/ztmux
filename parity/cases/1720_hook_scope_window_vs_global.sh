# Hooks live in the option sets, so a window-scoped hook only fires for that
# window while the global one fires for every window.
$TM set -g automatic-rename off
$TM set -g @global ''
$TM set -g @scoped ''
$TM new-window -d -n one
$TM new-window -d -n two
$TM set-hook -g after-select-window 'set -ga @global ",g"'
$TM set-hook -w -t one after-select-window 'set -ga @scoped ",w"'
$TM select-window -t one
$TM select-window -t two
$TM select-window -t one
echo "global=[$($TM show -gv @global)] scoped=[$($TM show -gv @scoped)]"
$TM set-hook -gu after-select-window
$TM set-hook -wu -t one after-select-window
