# -l sends the argument literally (no key-name lookup) and -H reads each argument
# as a hexadecimal byte (cmd-send-keys.c:36). Both land in the pane, so capture
# the pane to see what arrived.
$TM set -g status off
$TM split-window -d 'cat'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
$TM send-keys -t "$pane" -l 'C-a and Enter as text'
$TM send-keys -t "$pane" -H 21 21
for _ in 1 2 3 4 5 6 7 8 9 10; do
  out=$($TM capture-pane -p -t "$pane" | head -1)
  [ -n "$out" ] && break
  sleep 0.2
done
$TM capture-pane -p -t "$pane" | head -1
