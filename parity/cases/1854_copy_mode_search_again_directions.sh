# search-again repeats the last search in the same direction and search-reverse
# in the opposite one, so a forward search followed by a reverse walks back to
# where it started.
$TM set -g status off
$TM split-window -d "printf 'match\nfiller\nmatch\nfiller\nmatch\n'; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c match)" -ge 3 ] && break
  sleep 0.2
done
at() { $TM display-message -p -t "$pane" "  y=#{copy_cursor_y} line=[#{copy_cursor_line}]"; }
$TM copy-mode -t "$pane"
$TM send-keys -X -t "$pane" top-line
$TM send-keys -X -t "$pane" search-forward match
echo "first search:"; at
$TM send-keys -X -t "$pane" search-again
echo "search-again:"; at
$TM send-keys -X -t "$pane" search-again
echo "again:"; at
$TM send-keys -X -t "$pane" search-reverse
echo "search-reverse:"; at
echo "search formats: $($TM display-message -p -t "$pane" 'present=#{search_present} count=[#{search_count}]')"
$TM send-keys -X -t "$pane" cancel
