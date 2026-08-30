# select-pane -l returns to the previously active pane, #{pane_last} marks it,
# and killing that pane leaves the marker on something that still exists.
$TM split-window -d
$TM split-window -d
$TM select-pane -t 0
$TM select-pane -t 2
$TM list-panes -F '  #{pane_index} active=#{pane_active} last=#{pane_last}' | sort
$TM select-pane -l; echo "select -l rc=$?"
echo "after -l: active=$($TM display-message -p '#{pane_index}')"
$TM list-panes -F '  #{pane_index} active=#{pane_active} last=#{pane_last}' | sort
$TM kill-pane -t 2
echo "after killing the pane that was last:"
$TM list-panes -F '  #{pane_index} active=#{pane_active} last=#{pane_last}' | sort
$TM select-pane -l 2>&1; echo "select -l rc=$?"
