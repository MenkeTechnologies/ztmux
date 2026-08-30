# The *-and-cancel commands do their work and then leave copy mode
# (window-copy.c:3118 onwards). Each is checked for both halves: the buffer or
# the cursor moved, AND the pane is out of the mode afterwards.
$TM set -g status off
$TM split-window -d "printf 'alpha\nbravo\ncharlie\ndelta\n'; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 40); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c delta)" = 1 ] && break
  sleep 0.2
done
enter() { $TM send-keys -X -t "$pane" cancel 2>/dev/null; $TM copy-mode -t "$pane"; }

enter
$TM send-keys -X -t "$pane" top-line
$TM send-keys -X -t "$pane" copy-line-and-cancel
echo "copy-line-and-cancel: in_mode=$($TM display-message -p -t "$pane" '#{pane_in_mode}') buffer=[$($TM show-buffer)]"

enter
$TM send-keys -X -t "$pane" top-line
$TM send-keys -X -t "$pane" cursor-right
$TM send-keys -X -t "$pane" copy-end-of-line-and-cancel
echo "copy-end-of-line-and-cancel: in_mode=$($TM display-message -p -t "$pane" '#{pane_in_mode}') buffer=[$($TM show-buffer)]"

enter
$TM send-keys -X -t "$pane" top-line
$TM send-keys -X -t "$pane" begin-selection
$TM send-keys -X -t "$pane" cursor-down
$TM send-keys -X -t "$pane" copy-selection-and-cancel
echo "copy-selection-and-cancel: in_mode=$($TM display-message -p -t "$pane" '#{pane_in_mode}')"
$TM show-buffer | perl -pe 's/\s+$//' | sed 's/^/  /'

enter
$TM send-keys -X -t "$pane" cursor-down-and-cancel
echo "cursor-down-and-cancel: in_mode=$($TM display-message -p -t "$pane" '#{pane_in_mode}')"
