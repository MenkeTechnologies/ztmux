# pipe-no-clear pipes the selection without clearing it or copying to a buffer.
out="${TMPDIR:-/tmp}/ztpar_pipe_noclear.out"
command rm -f "$out"
$TM set -g status off
$TM split-window -d "printf 'sample text here\n'; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 40); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c sample)" = 1 ] && break
  sleep 0.2
done
$TM copy-mode -t "$pane"
$TM send-keys -X -t "$pane" top-line
$TM send-keys -X -t "$pane" begin-selection
$TM send-keys -X -t "$pane" end-of-line
$TM send-keys -X -t "$pane" pipe-no-clear "cat >> $out"
for _ in $(seq 1 40); do [ -s "$out" ] && break; sleep 0.2; done
echo "piped=[$(head -1 "$out" | perl -pe 's/\s+$//')]"
echo "selection still present=$($TM display-message -p -t "$pane" '#{selection_present}') in_mode=$($TM display-message -p -t "$pane" '#{pane_in_mode}')"
echo "buffers=[$($TM list-buffers -F '#{buffer_name}' 2>&1 | tr '\n' ' ')]"
command rm -f "$out"
