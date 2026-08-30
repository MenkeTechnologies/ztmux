# move-pane -b puts the source pane before the target instead of after, which
# changes the index order the layout is read back in.
$TM new-window -d -n src
$TM new-window -d -n dst
$TM split-window -d -t dst
$TM move-pane -s src.0 -t dst.0; echo "after rc=$?"
$TM list-panes -t dst -F '#{pane_index}:#{pane_height}' | sort
$TM new-window -d -n src2
$TM move-pane -b -s src2.0 -t dst.0; echo "before rc=$?"
$TM list-panes -t dst -F '#{pane_index}' | sort
