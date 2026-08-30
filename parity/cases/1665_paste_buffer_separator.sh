# paste-buffer -s sets the separator between lines and -d deletes the buffer
# after pasting (cmd-paste-buffer.c:36). The paste lands in the target pane, so
# read it back with capture-pane.
$TM set-buffer -b p 'aa
bb'
$TM split-window -d 'cat > /dev/null'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
echo "buffers before: $($TM list-buffers -F '#{buffer_name}' | sort | tr '\n' ' ')"
$TM paste-buffer -d -b p -s '+' -t "$pane"; echo "rc=$?"
echo "buffers after -d: $($TM list-buffers -F '#{buffer_name}' | sort | tr '\n' ' ')"
$TM paste-buffer -b nosuchbuffer 2>&1; echo "rc=$?"
