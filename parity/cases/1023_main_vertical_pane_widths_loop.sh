# per-pane widths of a main-vertical layout via the pane loop modifier
$TM set-window-option -g main-pane-width 30
$TM split-window -d
$TM split-window -d
$TM select-layout main-vertical
$TM display-message -p '#{P:#{pane_index}=#{pane_width} }'
