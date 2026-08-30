# clear-history -H clears the alternate screen's history as well as the main
# one, and copy-mode -d scrolls down one page as it enters (the mirror of -u).
$TM set -g status off
$TM split-window -d "i=1; while [ \$i -le 60 ]; do echo line \$i; i=\$((i+1)); done; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 25); do
  [ "$($TM display-message -p -t "$pane" '#{history_size}')" -ge 30 ] && break
  sleep 0.2
done
echo "history: $($TM display-message -p -t "$pane" '#{history_size}')"
$TM copy-mode -u -t "$pane"
echo "after -u: scroll=$($TM display-message -p -t "$pane" '#{scroll_position}')"
$TM copy-mode -d -t "$pane"
echo "after -d: scroll=$($TM display-message -p -t "$pane" '#{scroll_position}')"
$TM send-keys -X -t "$pane" cancel
$TM clear-history -H -t "$pane"; echo "clear-history -H rc=$?"
echo "history after: $($TM display-message -p -t "$pane" '#{history_size}')"
