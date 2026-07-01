# window loop sorted by name (W/n)
$TM new-window -d -n bbb
$TM new-window -d -n aaa
$TM display-message -p '#{W/n:#{window_name} }'
