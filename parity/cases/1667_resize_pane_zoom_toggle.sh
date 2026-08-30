# -Z toggles zoom: the window reports zoomed and the pane fills it, and toggling
# again restores every pane's size.
$TM split-window -d
before=$($TM list-panes -F '#{pane_index}:#{pane_height}' | sort | tr '\n' ' ')
echo "before: $before"
$TM resize-pane -Z; echo "zoom rc=$?"
$TM display-message -p 'zoomed=#{window_zoomed_flag} panes=#{window_panes}'
$TM list-panes -F '#{pane_index}:#{pane_height}:#{window_zoomed_flag}' | sort
$TM resize-pane -Z; echo "unzoom rc=$?"
after=$($TM list-panes -F '#{pane_index}:#{pane_height}' | sort | tr '\n' ' ')
echo "after: $after"
[ "$before" = "$after" ] && echo "sizes restored" || echo "sizes DIFFER"
