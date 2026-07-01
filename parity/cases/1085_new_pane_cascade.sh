# two floating panes cascade (4,2 then 8,4)
$TM new-pane -d "sleep 300"
$TM new-pane -d "sleep 300"
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
