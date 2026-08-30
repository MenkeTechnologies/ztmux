# copy-mode -u enters the mode already scrolled up one page, which is what the
# PPage binding does; without it the mode starts at the bottom.
$TM set -g status off
$TM split-window -d "i=1; while [ \$i -le 60 ]; do echo line \$i; i=\$((i+1)); done; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 25); do
  [ "$($TM display-message -p -t "$pane" '#{history_size}')" -ge 30 ] && break
  sleep 0.2
done
$TM copy-mode -t "$pane"
echo "plain:     scroll=$($TM display-message -p -t "$pane" '#{scroll_position}')"
$TM send-keys -X -t "$pane" cancel
$TM copy-mode -u -t "$pane"
echo "with -u:   scroll=$($TM display-message -p -t "$pane" '#{scroll_position}')"
echo "in mode:   $($TM display-message -p -t "$pane" '#{pane_in_mode}')"
$TM send-keys -X -t "$pane" cancel
