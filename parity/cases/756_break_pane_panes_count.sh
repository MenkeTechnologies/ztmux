# after breaking one pane out, the source window keeps the remaining two
$TM split-window -d
$TM split-window -d
$TM break-pane -d
$TM display-message -p '#{window_panes}'
