# Killing a pane in a zoomed window leaves the window unzoomed rather than
# zoomed on a pane that is gone.
$TM split-window -d
$TM split-window -d
$TM select-pane -t 1
$TM resize-pane -Z
$TM display-message -p 'before: zoomed=#{window_zoomed_flag} panes=#{window_panes} active=#{pane_index}'
$TM kill-pane -t 2
$TM display-message -p 'after killing another pane: zoomed=#{window_zoomed_flag} panes=#{window_panes}'
$TM resize-pane -Z 2>/dev/null
$TM display-message -p 'zoom again: zoomed=#{window_zoomed_flag}'
$TM kill-pane
$TM display-message -p 'after killing the zoomed pane: zoomed=#{window_zoomed_flag} panes=#{window_panes}'
