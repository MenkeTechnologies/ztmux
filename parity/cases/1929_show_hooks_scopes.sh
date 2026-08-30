# show-hooks takes the same scope flags as show-options: -g for global, -w for
# the window's own and -p for the pane's.
$TM set-hook -g after-new-window 'display-message g'
$TM set-hook -w after-select-pane 'display-message w'
$TM set-hook -p after-resize-pane 'display-message p'
echo "global:"; $TM show-hooks -g | grep -c '^after-new-window'
echo "window:"; $TM show-hooks -w | grep -c '^after-select-pane'
echo "pane:";   $TM show-hooks -p | grep -c '^after-resize-pane'
echo "== a scope that has none =="
$TM show-hooks -p | grep -c '^after-new-window'
echo "== show-environment -s reads the server set =="
$TM set-environment -h ZTPAR_SRV serverwide 2>/dev/null || true
$TM show-environment -s 2>&1 | head -2 | perl -pe 's/=.*/=VALUE/'
$TM set-hook -gu after-new-window; $TM set-hook -wu after-select-pane; $TM set-hook -pu after-resize-pane
