# window loop sorted by name reversed (W/nr)
$TM new-window -d -n bbb
$TM new-window -d -n aaa
$TM display-message -p '#{W/nr:#{window_name} }'
