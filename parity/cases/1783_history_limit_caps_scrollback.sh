# history-limit caps the scrollback: past it the oldest lines are dropped, so
# #{history_size} stops growing. clear-history empties it, and the limit is read
# per pane through #{history_limit}.
$TM set -g status off
$TM set -g history-limit 10
$TM split-window -d "i=1; while [ \$i -le 40 ]; do echo line \$i; i=\$((i+1)); done; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c 'line 40')" = 1 ] && break
  sleep 0.2
done
$TM display-message -p -t "$pane" 'limit=#{history_limit} size=#{history_size}'
echo "oldest line still in the history:"
$TM capture-pane -p -S -"$($TM display-message -p -t "$pane" '#{history_size}')" -t "$pane" | head -1
$TM clear-history -t "$pane"
$TM display-message -p -t "$pane" 'after clear-history: size=#{history_size}'
$TM set -gu history-limit
