# new-pane -L makes a tiled pane (no floating suffix in window_layout)
$TM new-pane -L -d "sleep 300"
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
