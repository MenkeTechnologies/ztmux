# While a pane is zoomed the window's VISIBLE layout is just that pane, while
# #{window_layout} still describes the real one underneath; unzooming brings
# them back together.
$TM split-window -d
$TM split-window -d
mask() { perl -pe 's/^[0-9a-f]{4},/CKSUM,/'; }
echo "unzoomed:"
echo "  layout:  $($TM display-message -p '#{window_layout}' | mask)"
echo "  visible: $($TM display-message -p '#{window_visible_layout}' | mask)"
$TM resize-pane -Z
echo "zoomed:"
echo "  zoomed flag: $($TM display-message -p '#{window_zoomed_flag}')"
echo "  layout:  $($TM display-message -p '#{window_layout}' | mask)"
echo "  visible: $($TM display-message -p '#{window_visible_layout}' | mask)"
$TM resize-pane -Z
echo "unzoomed again:"
echo "  layout:  $($TM display-message -p '#{window_layout}' | mask)"
echo "  visible: $($TM display-message -p '#{window_visible_layout}' | mask)"
