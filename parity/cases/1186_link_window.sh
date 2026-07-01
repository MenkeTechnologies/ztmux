$TM new-window -n orig
$TM link-window -s orig -t 8
$TM list-windows -F '#{window_index}:#{window_name}' -f '#{m:orig,#{window_name}}'
