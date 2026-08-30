# copy-mode -e leaves the mode as soon as scrolling reaches the bottom again,
# which is what the wheel bindings rely on; without -e the mode stays.
$TM set -g status off
$TM split-window -d "i=1; while [ \$i -le 40 ]; do echo line \$i; i=\$((i+1)); done; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 25); do
  [ "$($TM display-message -p -t "$pane" '#{history_size}')" -ge 20 ] && break
  sleep 0.2
done
$TM copy-mode -e -t "$pane"
$TM send-keys -X -t "$pane" page-up
echo "with -e, scrolled up: mode=[$($TM display-message -p -t "$pane" '#{pane_mode}')] scroll=$($TM display-message -p -t "$pane" '#{scroll_position}')"
$TM send-keys -X -t "$pane" page-down
$TM send-keys -X -t "$pane" page-down
for _ in $(seq 1 25); do
  [ -z "$($TM display-message -p -t "$pane" '#{pane_mode}')" ] && break
  sleep 0.2
done
echo "after scrolling back to the bottom: mode=[$($TM display-message -p -t "$pane" '#{pane_mode}')]"
$TM copy-mode -t "$pane"
$TM send-keys -X -t "$pane" page-up
$TM send-keys -X -t "$pane" page-down
$TM send-keys -X -t "$pane" page-down
echo "without -e, at the bottom:         mode=[$($TM display-message -p -t "$pane" '#{pane_mode}')]"
$TM send-keys -X -t "$pane" cancel
