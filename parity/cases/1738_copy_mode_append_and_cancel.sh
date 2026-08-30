# append-selection-and-cancel adds to the existing buffer instead of replacing
# it, and leaves the mode.
$TM set -g status off
$TM split-window -d "printf 'first line\nsecond line\n'; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 40); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c second)" = 1 ] && break
  sleep 0.2
done
$TM copy-mode -t "$pane"
$TM send-keys -X -t "$pane" top-line
$TM send-keys -X -t "$pane" begin-selection
$TM send-keys -X -t "$pane" end-of-line
$TM send-keys -X -t "$pane" copy-selection-and-cancel
echo "after copy:"; $TM show-buffer | perl -pe 's/\s+$//' | sed 's/^/  /'
$TM copy-mode -t "$pane"
$TM send-keys -X -t "$pane" top-line
$TM send-keys -X -t "$pane" cursor-down
$TM send-keys -X -t "$pane" begin-selection
$TM send-keys -X -t "$pane" end-of-line
$TM send-keys -X -t "$pane" append-selection-and-cancel
echo "after append: in_mode=$($TM display-message -p -t "$pane" '#{pane_in_mode}')"
$TM show-buffer | perl -pe 's/\s+$//' | sed 's/^/  /'
echo "buffer count=$($TM list-buffers -F '#{buffer_name}' | wc -l | tr -d ' ')"
