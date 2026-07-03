# pane_flags: last (-), active (*), and zoomed (Z).
$TM split-window "sleep 300"
$TM list-panes -F '#{pane_index}:#{pane_flags}'
$TM resize-pane -Z
$TM display-message -p 'active=#{pane_flags}'
