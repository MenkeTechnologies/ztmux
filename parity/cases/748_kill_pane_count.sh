# four panes minus one killed leaves three
$TM split-window -d
$TM split-window -d
$TM split-window -d
$TM kill-pane -t 2
$TM display-message -p '#{window_panes}'
