# word-separators decides what counts as a word boundary for the copy-mode word
# motions; changing it changes where next-word lands.
$TM set -g status off
$TM split-window -d "printf 'alpha-beta gamma_delta\n'; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 40); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c gamma)" = 1 ] && break
  sleep 0.2
done
probe() {
  $TM send-keys -X -t "$pane" cancel 2>/dev/null
  $TM copy-mode -t "$pane"
  $TM send-keys -X -t "$pane" top-line
  $TM send-keys -X -t "$pane" start-of-line
  $TM send-keys -X -t "$pane" next-word
  $TM display-message -p -t "$pane" "  next-word -> x=#{copy_cursor_x}"
  $TM send-keys -X -t "$pane" next-word
  $TM display-message -p -t "$pane" "  again     -> x=#{copy_cursor_x}"
}
echo "default separators [$($TM show -gv word-separators | cat -v)]:"
probe
$TM set -g word-separators ' -_'
echo "with ' -_' as separators:"
probe
$TM set -gu word-separators
