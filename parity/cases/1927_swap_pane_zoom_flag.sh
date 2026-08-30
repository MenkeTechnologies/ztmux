# swap-pane -Z keeps the window zoomed across the swap, and -d leaves the active
# pane where it was.
$TM split-window -d
$TM split-window -d
$TM select-pane -t 0
$TM resize-pane -Z
echo "before: zoomed=$($TM display-message -p '#{window_zoomed_flag}') active=$($TM display-message -p '#{pane_index}')"
$TM swap-pane -Z -s 0 -t 2; echo "-Z rc=$?"
echo "after -Z: zoomed=$($TM display-message -p '#{window_zoomed_flag}') active=$($TM display-message -p '#{pane_index}')"
$TM resize-pane -Z 2>/dev/null
$TM swap-pane -s 0 -t 1; echo "plain rc=$?"
echo "after plain: zoomed=$($TM display-message -p '#{window_zoomed_flag}')"
