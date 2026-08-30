# wrap-search decides whether a copy-mode search that runs off the end starts
# again at the other one. With it off the cursor stays put and the search
# reports no match.
$TM set -g status off
$TM split-window -d "printf 'needle\nfiller\nfiller\n'; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 40); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c needle)" = 1 ] && break
  sleep 0.2
done
probe() {
  $TM send-keys -X -t "$pane" cancel 2>/dev/null
  $TM copy-mode -t "$pane"
  $TM send-keys -X -t "$pane" history-bottom
  $TM send-keys -X -t "$pane" search-forward needle
  $TM display-message -p -t "$pane" "  y=#{copy_cursor_y} line=[#{copy_cursor_line}] present=#{search_present}"
}
$TM setw -g wrap-search on
echo "wrap-search on:"; probe
$TM setw -g wrap-search off
echo "wrap-search off:"; probe
$TM setw -gu wrap-search
