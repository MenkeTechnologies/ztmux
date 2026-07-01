# a horizontal then a vertical split yields three panes
$TM split-window -h -d
$TM split-window -v -d
$TM display-message -p '#{window_panes}'
