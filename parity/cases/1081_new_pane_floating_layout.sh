# new-pane creates a floating pane (window_layout has the <...> floating suffix)
$TM new-pane -d "sleep 300"
$TM display-message -p '#{window_layout}' | sed 's/^[0-9a-f]*,//'
