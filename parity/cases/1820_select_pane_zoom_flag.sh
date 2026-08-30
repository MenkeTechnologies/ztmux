# select-pane -Z zooms the pane it selects, which is the shorthand for a select
# followed by resize-pane -Z; selecting another pane without -Z leaves the zoom
# behind.
$TM split-window -d
$TM split-window -d
$TM select-pane -t 0
$TM display-message -p 'zoomed at rest: #{window_zoomed_flag}'
$TM select-pane -Z -t 1; echo "select -Z rc=$?"
$TM display-message -p 'after select -Z: active=#{pane_index} zoomed=#{window_zoomed_flag}'
$TM list-panes -F '  #{pane_index}:#{pane_height}' | sort
$TM select-pane -t 2
$TM display-message -p 'after selecting another pane: active=#{pane_index} zoomed=#{window_zoomed_flag}'
$TM resize-pane -Z 2>/dev/null
$TM display-message -p 'after toggling zoom off: zoomed=#{window_zoomed_flag}'
