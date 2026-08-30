# The copy-pipe family sends the copied text to a command as well as to a
# buffer; the -no-clear forms keep the selection, and the line/end-of-line forms
# choose what is copied without a selection existing.
out="${TMPDIR:-/tmp}/ztpar_copy_pipe.out"
command rm -f "$out"
$TM set -g status off
$TM split-window -d "printf 'one two three\nfour five six\n'; sleep 300"
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 40); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c four)" = 1 ] && break
  sleep 0.2
done
enter() { $TM send-keys -X -t "$pane" cancel 2>/dev/null; $TM copy-mode -t "$pane"; $TM send-keys -X -t "$pane" top-line; }

enter
$TM send-keys -X -t "$pane" copy-pipe-line "cat >> $out"
for _ in $(seq 1 40); do [ -s "$out" ] && break; sleep 0.2; done
echo "copy-pipe-line wrote: [$(head -1 "$out" | perl -pe 's/\s+$//')] in_mode=$($TM display-message -p -t "$pane" '#{pane_in_mode}')"

command rm -f "$out"
enter
$TM send-keys -X -t "$pane" cursor-right
$TM send-keys -X -t "$pane" copy-pipe-end-of-line-and-cancel "cat >> $out"
for _ in $(seq 1 40); do [ -s "$out" ] && break; sleep 0.2; done
echo "copy-pipe-end-of-line-and-cancel wrote: [$(head -1 "$out" | perl -pe 's/\s+$//')] in_mode=$($TM display-message -p -t "$pane" '#{pane_in_mode}')"

command rm -f "$out"
enter
$TM send-keys -X -t "$pane" begin-selection
$TM send-keys -X -t "$pane" cursor-right
$TM send-keys -X -t "$pane" copy-pipe-no-clear "cat >> $out"
for _ in $(seq 1 40); do [ -s "$out" ] && break; sleep 0.2; done
echo "copy-pipe-no-clear wrote: [$(head -1 "$out" | perl -pe 's/\s+$//')] selection_present=$($TM display-message -p -t "$pane" '#{selection_present}')"

command rm -f "$out"
enter
$TM send-keys -X -t "$pane" copy-pipe-line-and-cancel "cat >> $out"
for _ in $(seq 1 40); do [ -s "$out" ] && break; sleep 0.2; done
echo "copy-pipe-line-and-cancel wrote: [$(head -1 "$out" | perl -pe 's/\s+$//')] in_mode=$($TM display-message -p -t "$pane" '#{pane_in_mode}')"
command rm -f "$out"
