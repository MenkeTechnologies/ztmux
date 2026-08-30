# move-window -d leaves the current window selected. move-pane in next-3.7 is no
# longer the plain join: unless -M is given it requires the TARGET pane to be
# floating and says "pane is not floating" otherwise (cmd-join-pane.c:388-395),
# so joining into a tiled pane is join-pane's job, where -d leaves the moved pane
# unselected and -v splits top-to-bottom.
$TM set -g automatic-rename off
$TM set -g status off
$TM new-window -d -n one 'sleep 300'
$TM new-window -d -n two 'sleep 300'
echo "current before: $($TM display-message -p '#{window_name}')"
$TM move-window -d -s one -t 9; echo "move-window -d rc=$?"
echo "current after:  $($TM display-message -p '#{window_name}')"
echo "moved to index: $($TM list-windows -F '#{window_index}:#{window_name}' | grep ':one$')"
echo "== move-pane wants a floating target =="
$TM move-pane -d -s 9.0 -t two.0 -v 2>&1; echo "rc=$?"
echo "two still has $($TM list-panes -t two | wc -l | tr -d ' ') pane"
echo "== join-pane -d -v does the move, and leaves pane 0 active =="
$TM join-pane -d -v -s 9.0 -t two.0; echo "rc=$?"
echo "two now has $($TM list-panes -t two | wc -l | tr -d ' ') panes, active index $($TM display-message -p -t two '#{pane_index}')"
echo "the split was vertical: $($TM list-panes -t two -F '#{pane_width}' | sort -u | wc -l | tr -d ' ') distinct width"
echo "window 9 is gone: [$($TM list-windows -F '#{window_index}' | grep -c '^9$')]"
echo "== without -d the moved pane becomes active =="
$TM new-window -d -n three 'sleep 300'
$TM join-pane -v -s three.0 -t two.0; echo "rc=$?"
echo "active index in two: $($TM display-message -p -t two '#{pane_index}')"
