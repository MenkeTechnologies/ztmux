# The after-<command> hooks fire once the command has run (options-table.c:1881
# onwards). The hook body is a command list like any other, so it can record
# what it saw through an option.
$TM set -g @log ''
$TM set-hook -g after-kill-pane 'set -ga @log ",kill"'
$TM set-hook -g after-new-window 'set -ga @log ",neww"'
$TM set-hook -g after-split-window 'set -ga @log ",split"'
$TM new-window -d -n hooked
$TM split-window -d -t hooked
$TM kill-pane -t hooked.1
echo "log=[$($TM show -gv @log)]"
$TM set-hook -gu after-kill-pane
$TM set-hook -gu after-new-window
$TM set-hook -gu after-split-window
$TM set -g @log ''
$TM new-window -d
echo "after unsetting: [$($TM show -gv @log)]"
