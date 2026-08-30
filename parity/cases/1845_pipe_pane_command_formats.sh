# The command pipe-pane runs is a format, so it can name the pane it belongs to;
# the expansion happens once, when the pipe is set up.
out="${TMPDIR:-/tmp}/ztpar_pipe_fmt"
command rm -f "$out".*
$TM set -g status off
$TM split-window -d 'cat'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
idx=$($TM display-message -p -t "$pane" '#{pane_index}')
$TM pipe-pane -t "$pane" "cat >> $out.#{pane_index}"
$TM display-message -p -t "$pane" 'piped=#{pane_pipe}'
$TM send-keys -t "$pane" -l 'through-the-pipe'
$TM send-keys -t "$pane" Enter
for _ in $(seq 1 25); do
  [ -s "$out.$idx" ] && break
  sleep 0.2
done
echo "file named after the pane index exists: $([ -f "$out.$idx" ] && echo yes || echo no)"
grep -c through-the-pipe "$out.$idx"
$TM pipe-pane -t "$pane"
command rm -f "$out".*
