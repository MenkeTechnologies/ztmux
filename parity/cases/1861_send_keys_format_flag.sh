# send-keys -F expands each argument as a format before sending it, so a key
# argument can be computed; without -F the text goes as typed.
$TM set -g status off
$TM split-window -d 'cat'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
$TM send-keys -t "$pane" -l -F 'windows=#{session_windows}'
$TM send-keys -t "$pane" Enter
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c 'windows=')" -ge 1 ] && break
  sleep 0.2
done
echo "with -F, the format was expanded: $($TM capture-pane -p -t "$pane" | grep -c 'windows=1')"
$TM send-keys -t "$pane" -l 'raw=#{session_windows}'
$TM send-keys -t "$pane" Enter
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c 'raw=')" -ge 1 ] && break
  sleep 0.2
done
echo "without -F, the text went as typed: $($TM capture-pane -p -t "$pane" | grep -cF 'raw=#{session_windows}')"
