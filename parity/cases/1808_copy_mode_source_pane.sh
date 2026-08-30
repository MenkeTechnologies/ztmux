# copy-mode -s shows ANOTHER pane's contents in this pane's copy mode, so the
# cursor line comes from the source pane while the mode belongs to the target.
$TM set -g status off
$TM split-window -d "printf 'from-the-source\n'; sleep 300"
src=$($TM list-panes -F '#{pane_id}' | tail -1)
dst=$($TM list-panes -F '#{pane_id}' | head -1)
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$src" | grep -c from-the-source)" = 1 ] && break
  sleep 0.2
done
$TM copy-mode -s "$src" -t "$dst"
echo "target in mode: $($TM display-message -p -t "$dst" '#{pane_in_mode}') source in mode: $($TM display-message -p -t "$src" '#{pane_in_mode}')"
$TM send-keys -X -t "$dst" top-line
echo "line under the cursor: [$($TM display-message -p -t "$dst" '#{copy_cursor_line}')]"
$TM send-keys -X -t "$dst" cancel
echo "after cancel: $($TM display-message -p -t "$dst" '#{pane_in_mode}')"
echo "== and without -s it shows its own contents =="
$TM copy-mode -t "$dst"
$TM send-keys -X -t "$dst" top-line
echo "line under the cursor: [$($TM display-message -p -t "$dst" '#{copy_cursor_line}')]"
$TM send-keys -X -t "$dst" cancel
