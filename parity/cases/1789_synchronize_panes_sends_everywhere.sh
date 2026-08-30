# synchronize-panes sends what is typed into one pane to every pane in the
# window; with it off only the target pane receives.
$TM set -g status off
$TM split-window -d 'cat'
$TM split-window -d 'cat'
first=$($TM list-panes -F '#{pane_id}' | head -1)
settle() { for _ in $(seq 1 40); do [ -n "$($TM capture-pane -p -t "$1" | head -1)" ] && return; sleep 0.2; done; }
echo "== off: only the target sees it =="
$TM send-keys -t "$first" -l 'only-here'
$TM send-keys -t "$first" Enter
settle "$first"
$TM list-panes -F '#{pane_index}' | while read i; do
  printf '  pane %s: [%s]\n' "$i" "$($TM capture-pane -p -t "$i" | head -1)"
done
echo "== on: every pane sees it =="
$TM setw synchronize-panes on
$TM send-keys -t "$first" -l 'everywhere'
$TM send-keys -t "$first" Enter
for _ in $(seq 1 40); do
  [ "$($TM capture-pane -p -t 2 | grep -c everywhere)" -ge 1 ] && break
  sleep 0.2
done
$TM list-panes -F '#{pane_index}' | while read i; do
  printf '  pane %s: [%s]\n' "$i" "$($TM capture-pane -p -t "$i" | grep -c everywhere)"
done
$TM setw -u synchronize-panes
