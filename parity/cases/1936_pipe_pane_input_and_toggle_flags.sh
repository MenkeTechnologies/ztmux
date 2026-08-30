# pipe-pane: with no command the pipe is only torn down; -o is a TOGGLE rather
# than a guard, because the old pipe is destroyed before -o is looked at, so -o
# over an open pipe closes it and opens nothing (cmd-pipe-pane.c:77-100); -I
# feeds the pane's input instead of its output, and a dead pane is an error.
out="${TMPDIR:-/tmp}/ztpar_pipepane.out"
command rm -f "$out" "$out.second"
$TM set -g status off
pane=$($TM display-message -p '#{pane_id}')
echo "piped before: $($TM display-message -p '#{pane_pipe}')"
$TM pipe-pane -t "$pane" "cat >> $out"; echo "open rc=$?"
echo "piped now:    $($TM display-message -p '#{pane_pipe}')"
echo "== -o over an open pipe closes it and opens nothing =="
$TM pipe-pane -o -t "$pane" "cat >> $out.second"; echo "rc=$?"
echo "piped after:  $($TM display-message -p '#{pane_pipe}')"
echo "second file was never made: $([ -e "$out.second" ] && echo yes || echo no)"
echo "== no command tears the pipe down =="
$TM pipe-pane -t "$pane"; echo "rc=$?"
echo "piped after:  $($TM display-message -p '#{pane_pipe}')"
echo "== an empty command is the same as none =="
$TM pipe-pane -t "$pane" ''; echo "rc=$?"
echo "piped after:  $($TM display-message -p '#{pane_pipe}')"
echo "== -I pipes the pane's input, and -o with no pipe open does open one =="
# The pipe command has to outlive the check: with -I the child's stdin is
# /dev/null, so a command that reads to EOF exits at once and the pipe is torn
# down by the error callback -- at a moment neither binary has to agree on.
$TM pipe-pane -I -o -t "$pane" 'sleep 300'; echo "rc=$?"
echo "piped:        $($TM display-message -p '#{pane_pipe}')"
$TM pipe-pane -t "$pane"
echo "== a pane that has exited is an error =="
$TM new-window -d -n dead 'exit 0'
$TM set -g remain-on-exit on
$TM new-window -d -n dead2 'exit 0'
for _ in $(seq 1 25); do
  [ "$($TM display-message -p -t dead2 '#{pane_dead}')" = 1 ] && break
  sleep 0.2
done
$TM pipe-pane -t dead2 'cat > /dev/null' 2>&1; echo "rc=$?"
