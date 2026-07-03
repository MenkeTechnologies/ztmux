# kill-window -a -f (filter, -a only) + -f validation.
$TM new-window -n keep "sleep 300"
$TM new-window -n gone1 "sleep 300"
$TM new-window -n gone2 "sleep 300"
$TM kill-window -a -f '#{m:gone*,#{window_name}}' -t keep
$TM list-windows -F '#{window_name}'
$TM kill-window -f x
