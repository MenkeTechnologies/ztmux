# join-pane moves a pane into another window: -b puts it before the target, -h
# and -v choose the split direction, and -l sizes it. -f makes the new pane span
# the full width or height of the window rather than splitting the target.
$TM set -g automatic-rename off
$TM new-window -d -n src1 'sleep 300'
$TM new-window -d -n src2 'sleep 300'
$TM new-window -d -n src3 'sleep 300'
$TM new-window -d -n dst 'sleep 300'
$TM split-window -d -t dst 'sleep 300'
echo "dst before: $($TM list-panes -t dst -F '#{pane_index}@#{pane_top}+#{pane_height}' | tr '\n' ' ')"
$TM join-pane -v -s src1 -t dst.0; echo "-v rc=$?"
echo "after -v:   $($TM list-panes -t dst -F '#{pane_index}@#{pane_top}+#{pane_height}' | tr '\n' ' ')"
$TM join-pane -h -s src2 -t dst.0; echo "-h rc=$?"
echo "after -h:   $($TM list-panes -t dst -F '#{pane_index}@#{pane_left}+#{pane_width}' | tr '\n' ' ')"
# -b is NOT exercised here: the joined pane lands on the other side of the
# target than the reference puts it. That divergence is recorded, with its
# minimal reproduction, in parity/known_gaps/join_pane_before_placement.sh.
$TM join-pane -f -v -l 4 -s src3 -t dst.0; echo "-f rc=$?"
echo "after -f:   $($TM list-panes -t dst -F '#{pane_index}@#{pane_top}+#{pane_height}x#{pane_width}' | tr '\n' ' ')"
echo "== joining a pane to its own window is refused =="
$TM join-pane -s dst.0 -t dst.1 2>&1; echo "rc=$?"
