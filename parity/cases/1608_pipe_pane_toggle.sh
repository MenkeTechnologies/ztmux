# pipe-pane starts a pipe (#{pane_pipe} = 1), a bare pipe-pane stops it, and -o
# toggles: the same command twice ends with the pipe off.
out="${TMPDIR:-/tmp}/ztpar_pipe_pane.out"
command rm -f "$out"
$TM display-message -p 'before=#{pane_pipe}'
$TM pipe-pane "cat > $out"
$TM display-message -p 'after=#{pane_pipe}'
$TM pipe-pane
$TM display-message -p 'stopped=#{pane_pipe}'
$TM pipe-pane -o "cat > $out"
$TM display-message -p 'toggle_on=#{pane_pipe}'
$TM pipe-pane -o "cat > $out"
$TM display-message -p 'toggle_off=#{pane_pipe}'
command rm -f "$out"
