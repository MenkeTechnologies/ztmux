# Every command that has an after-<name> hook must fire it. Rather than one case
# per hook, each is armed to append its own name to an option and then the
# command is run; the log is the list of hooks that fired, in order.
$TM set -g @log ''
# The hooks are named in full so the corpus can be grepped for them.
HOOKS='after-bind-key after-capture-pane after-copy-mode after-display-message
after-list-buffers after-list-clients after-list-keys after-list-panes
after-list-sessions after-list-windows after-load-buffer after-paste-buffer
after-pipe-pane after-refresh-client after-resize-pane after-resize-window
after-save-buffer after-select-layout after-send-keys after-set-buffer
after-set-environment after-set-option after-show-environment
after-show-messages after-show-options after-unbind-key'
for h in $HOOKS; do $TM set-hook -g "$h" "set -ga @log \",${h#after-}\""; done

d=$(mktemp -d)
$TM bind -T ztpar q display-message hi >/dev/null
$TM capture-pane -b cap >/dev/null
$TM copy-mode; $TM send-keys -X cancel
$TM display-message -p hi >/dev/null
$TM list-buffers >/dev/null
$TM list-clients >/dev/null
$TM list-keys -T ztpar >/dev/null
$TM list-panes >/dev/null
$TM list-sessions >/dev/null
$TM list-windows >/dev/null
$TM set-buffer -b lb 'x' >/dev/null
$TM save-buffer -b lb "$d/b.txt" >/dev/null
$TM load-buffer -b lb2 "$d/b.txt" >/dev/null
$TM paste-buffer -b lb2 >/dev/null
$TM pipe-pane >/dev/null
$TM refresh-client >/dev/null 2>&1
$TM resize-pane -y 20 >/dev/null 2>&1
$TM resize-window -x 80 >/dev/null 2>&1
$TM select-layout even-vertical >/dev/null 2>&1
$TM set-environment FOO bar >/dev/null
$TM set-option -g @opt v >/dev/null
$TM show-environment >/dev/null
$TM show-messages >/dev/null 2>&1
$TM show-options -g >/dev/null
$TM unbind -T ztpar q >/dev/null
command rm -rf "$d"

$TM show -gv @log | tr ',' '\n' | grep -v '^$' | sort | uniq -c | perl -pe 's/^\s+//'
for h in $HOOKS; do $TM set-hook -gu "$h"; done
