# -N repeats the key that many times.
$TM set -g status off
$TM split-window -d 'cat'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
$TM send-keys -t "$pane" -N 5 -l 'z'
for _ in 1 2 3 4 5 6 7 8 9 10; do
  out=$($TM capture-pane -p -t "$pane" | head -1)
  [ -n "$out" ] && break
  sleep 0.2
done
$TM capture-pane -p -t "$pane" | head -1
