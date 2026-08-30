# The incremental searches are built for the interactive prompt: they update as
# the string grows, and driven from a command line -- with no prompt state
# behind them -- they leave the cursor where it was. The plain
# search-backward-text does move it, and the search formats follow. This case
# pins that difference, which is easy to get wrong in either direction.
$TM set -g status off
$TM split-window -d "printf 'alpha\nbravo\ncharlie\ndelta\necho\n'; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 40); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c echo)" -ge 1 ] && break
  sleep 0.2
done
$TM copy-mode -t "$pane"
$TM send-keys -X -t "$pane" top-line
echo "start: $($TM display-message -p -t "$pane" 'y=#{copy_cursor_y} line=[#{copy_cursor_line}]')"
$TM send-keys -X -t "$pane" search-forward-incremental 'charlie'
echo "search-forward-incremental charlie: $($TM display-message -p -t "$pane" 'y=#{copy_cursor_y} line=[#{copy_cursor_line}]')"
$TM send-keys -X -t "$pane" search-backward-incremental 'alpha'
echo "search-backward-incremental alpha: $($TM display-message -p -t "$pane" 'y=#{copy_cursor_y} line=[#{copy_cursor_line}]')"
$TM send-keys -X -t "$pane" search-backward-text 'bravo'
echo "search-backward-text bravo: $($TM display-message -p -t "$pane" 'y=#{copy_cursor_y} line=[#{copy_cursor_line}]')"
echo "search formats: $($TM display-message -p -t "$pane" 'match=#{search_present} count=[#{search_count}]')"
