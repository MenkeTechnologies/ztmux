# window-linked / window-unlinked and session-created / session-closed fire on
# the object's lifecycle rather than on a command name.
$TM set -g @log ''
$TM set-hook -g window-linked 'set -ga @log ",linked"'
$TM set-hook -g window-unlinked 'set -ga @log ",unlinked"'
$TM set-hook -g session-created 'set -ga @log ",screated"'
$TM set-hook -g session-closed 'set -ga @log ",sclosed"'
$TM new-session -d -s hooked -x 80 -y 24
$TM new-window -d -t hooked -n extra
$TM kill-window -t hooked:extra
$TM kill-session -t hooked
echo "log=[$($TM show -gv @log)]"
for h in window-linked window-unlinked session-created session-closed; do $TM set-hook -gu "$h"; done
