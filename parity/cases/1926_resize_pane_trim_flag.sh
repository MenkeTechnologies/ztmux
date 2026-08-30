# resize-pane -T trims the lines below the cursor into the history rather than
# resizing anything (cmd-resize-pane.c:71-75), and does nothing at all while the
# pane is in a mode.
$TM set -g status off
$TM split-window -d "printf 'one\ntwo\nthree\n'; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c three)" = 1 ] && break
  sleep 0.2
done
# A pane that has not printed yet would make the trim vacuous, so say so once.
[ "$($TM capture-pane -p -t "$pane" | grep -c three)" = 1 ] || {
  echo "pane produced no output in time"; exit 0; }
echo "before: history=$($TM display-message -p -t "$pane" '#{history_size}') cursor_y=$($TM display-message -p -t "$pane" '#{cursor_y}')"
$TM resize-pane -T -t "$pane"; echo "-T rc=$?"
echo "after:  history=$($TM display-message -p -t "$pane" '#{history_size}') cursor_y=$($TM display-message -p -t "$pane" '#{cursor_y}')"
$TM capture-pane -p -t "$pane" | grep -c . 
echo "== in a mode it does nothing =="
$TM copy-mode -t "$pane"
$TM resize-pane -T -t "$pane"; echo "rc=$?"
echo "history unchanged: $($TM display-message -p -t "$pane" '#{history_size}')"
$TM send-keys -X -t "$pane" cancel
