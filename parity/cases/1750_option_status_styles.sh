# status-bg / status-fg are the old split of status-style, and the per-side and
# per-state styles sit beside them. All are style-typed strings.
for o in status-bg status-fg status-left-style status-right-style \
         window-status-activity-style window-status-bell-style message-line; do
  printf '%-30s %s\n' "$o" "$($TM show -gv "$o" 2>&1)"
done
echo "== setting them =="
$TM set -g status-bg blue; $TM show -gv status-bg
$TM set -g status-fg colour200; $TM show -gv status-fg
$TM set -g status-left-style 'bold,fg=red'; $TM show -gv status-left-style
$TM set -g window-status-bell-style 'reverse'; $TM show -gv window-status-bell-style
echo "== message-line is a choice =="
$TM set -g message-line 2; $TM show -gv message-line
$TM set -g message-line 9 2>&1; echo "rc=$?"
echo "== a style option rejects nonsense =="
$TM set -g status-left-style 'notastyle' 2>&1; echo "rc=$?"
