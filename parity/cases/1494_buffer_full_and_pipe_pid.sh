# Two format callbacks that only resolve to anything under the right format tree.
#
# `#{buffer_full}` needs a paste buffer in the tree, so it is empty from
# display-message and only meaningful through `list-buffers -F`. It is the
# unabridged buffer contents, where `#{buffer_sample}` is the truncated one --
# a port that wires buffer_full to the sample callback passes every
# display-message probe and still returns the wrong string here.
$TM set-buffer -b one 'alpha beta'
$TM set-buffer -b two $'line1\nline2'
$TM set-buffer -b three '   leading and trailing   '
$TM list-buffers -F '#{buffer_name} full=[#{buffer_full}] sample=[#{buffer_sample}] size=#{buffer_size}'
# `#{pane_pipe_pid}` is the pid of the pipe-pane child, so its value is not
# reproducible -- the test is the transition, which is what the field being
# absent from window_pane would break.
$TM display-message -p 'before=[#{?pane_pipe_pid,set,unset}]'
$TM pipe-pane 'cat >/dev/null'
$TM display-message -p 'during=[#{?pane_pipe_pid,set,unset}]'
$TM pipe-pane
$TM display-message -p 'after =[#{?pane_pipe_pid,set,unset}]'
