# pane-died fires when a pane's process exits while remain-on-exit keeps the
# pane; pane-exited fires when the pane goes away with it.
$TM set -g @log ''
$TM set-hook -g pane-died 'set -ga @log ",died"'
$TM set-hook -g pane-exited 'set -ga @log ",exited"'
$TM setw -g remain-on-exit on
$TM split-window -d 'true'
for _ in $(seq 1 40); do
  [ "$($TM list-panes -F '#{pane_dead}' | grep -c '^1$')" = 1 ] && break
  sleep 0.2
done
echo "with remain-on-exit: [$($TM show -gv @log)]"
$TM kill-pane -t 1
$TM setw -g remain-on-exit off
$TM set -g @log ''
$TM split-window -d 'true'
for _ in $(seq 1 40); do
  [ "$($TM list-panes -F '#{pane_index}' | wc -l | tr -d ' ')" = 1 ] && break
  sleep 0.2
done
echo "without remain-on-exit: [$($TM show -gv @log)]"
for h in pane-died pane-exited; do $TM set-hook -gu "$h"; done
