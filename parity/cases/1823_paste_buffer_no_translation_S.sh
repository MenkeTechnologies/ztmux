# paste-buffer -S writes the buffer straight to the pane without the key
# translation the normal path applies, so control characters arrive as
# themselves. Read back from a pane running cat -v, which makes them visible.
$TM set -g status off
$TM set-buffer -b ctl "$(printf 'before\tafter')"
$TM split-window -d 'cat -v'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
$TM paste-buffer -b ctl -t "$pane"
$TM send-keys -t "$pane" Enter
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c after)" -ge 1 ] && break
  sleep 0.2
done
echo "default paste:"
$TM capture-pane -p -t "$pane" | head -2 | sed 's/^/  /'
$TM set-buffer -b ctl2 "$(printf 'raw\tpaste')"
$TM paste-buffer -S -b ctl2 -t "$pane"
$TM send-keys -t "$pane" Enter
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c paste)" -ge 1 ] && break
  sleep 0.2
done
echo "with -S:"
$TM capture-pane -p -t "$pane" | head -4 | sed 's/^/  /'
