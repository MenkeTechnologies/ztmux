# Killing a pane collapses its layout cell into its parent and every surviving
# pane's offset is recomputed from the parent's. That recomputation is the one
# that produced a crash at negative offsets once already, so the full offset set
# (left/top/right/bottom plus the at_* edge flags) is compared after each kill
# rather than just the pane count.
$TM new-window -d -n ko 'sleep 300'
$TM split-window -d -t ko 'sleep 300'
$TM split-window -d -h -t ko 'sleep 300'
$TM split-window -d -t ko.0 'sleep 300'
$TM split-window -d -h -t ko.2 'sleep 300'
edges() {
  $TM list-panes -t ko -F \
    "$1 #{pane_index} #{pane_width}x#{pane_height} l=#{pane_left} t=#{pane_top} r=#{pane_right} b=#{pane_bottom} L#{pane_at_left}T#{pane_at_top}R#{pane_at_right}B#{pane_at_bottom}"
}
edges five
$TM kill-pane -t ko.1; edges after-kill-1
$TM kill-pane -t ko.2; edges after-kill-2
$TM kill-pane -t ko.0; edges after-kill-0
$TM display-message -p -t ko 'panes=#{window_panes} layout=#{window_layout}'
# The last pane in a window cannot leave a zero-cell layout behind.
$TM kill-pane -t ko.0 2>&1
$TM list-windows -F '#{window_name} #{window_panes}'
