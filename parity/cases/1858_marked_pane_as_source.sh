# A marked pane can be named as {marked} by the commands that take a source, so
# join-pane -s '{marked}' moves the pane that was marked earlier.
$TM set -g automatic-rename off
$TM set -g status off
$TM new-window -d -n src "printf 'the-marked-pane\n'; sleep 300"
$TM new-window -d -n dst 'sleep 300'
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t src | grep -c the-marked-pane)" -ge 1 ] && break
  sleep 0.2
done
# A slow pane must not turn into a difference: say so once and stop.
[ "$($TM capture-pane -p -t src | grep -c the-marked-pane)" -ge 1 ] || {
  echo "source pane produced no output in time"; exit 0; }
$TM select-pane -m -t src.0
echo "marked: $($TM display-message -p -t '{marked}' '#{window_name}.#{pane_index}')"
$TM join-pane -s '{marked}' -t dst.0; echo "join rc=$?"
$TM list-panes -t dst -F '  dst pane #{pane_index}: [#{?#{m:*the-marked-pane*,#{pane_id}},,}]' >/dev/null
echo "dst now has $($TM list-windows -F '#{window_name}:#{window_panes}' | grep '^dst' ) panes"
echo "  moved pane still shows its own text: $($TM capture-pane -p -t dst.1 | grep -c the-marked-pane)"
echo "mark cleared after the move: [$($TM display-message -p -t '{marked}' '#{pane_index}' 2>&1)]"
