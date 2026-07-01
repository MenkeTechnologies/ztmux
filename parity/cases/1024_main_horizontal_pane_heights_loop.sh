# per-pane heights of a main-horizontal layout via the pane loop modifier
$TM set-window-option -g main-pane-height 10
$TM split-window -d
$TM split-window -d
$TM select-layout main-horizontal
$TM display-message -p '#{P:#{pane_index}=#{pane_height} }'
