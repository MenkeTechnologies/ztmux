$TM new-session -d -s extra -x 80 -y 24
$TM list-sessions -F '#{session_name}' -f '#{m:extra,#{session_name}}'
