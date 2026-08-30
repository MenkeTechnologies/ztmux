# -O pipes the pane's output (the default) and -I pipes input into the pane;
# #{pane_pipe} is set for either. Poll for the sentinel rather than for the file
# merely being non-empty, so the count is not read mid-write.
out="${TMPDIR:-/tmp}/ztpar_pipe_io.out"
command rm -f "$out"
$TM set -g status off
$TM split-window -d 'cat'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
$TM pipe-pane -O -t "$pane" "cat >> $out"
$TM display-message -p -t "$pane" 'piped=#{pane_pipe}'
$TM send-keys -t "$pane" -l 'hello'
$TM send-keys -t "$pane" Enter
# `cat` echoes the line back, so the pipe sees it twice: once as the terminal
# echo of the typed characters and once as the program's own output.
for _ in $(seq 1 40); do
  [ "$(grep -c hello "$out" 2>/dev/null)" = 2 ] && break
  sleep 0.2
done
grep -c hello "$out"
$TM pipe-pane -t "$pane"
$TM display-message -p -t "$pane" 'stopped=#{pane_pipe}'
echo "== -I and -O together =="
$TM pipe-pane -I -O -t "$pane" "cat > /dev/null"; echo "rc=$?"
$TM display-message -p -t "$pane" 'both=#{pane_pipe}'
command rm -f "$out"
