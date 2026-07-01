$TM new-session -d -s a1 -x 80 -y 24
$TM new-session -d -s a2 -x 80 -y 24
$TM kill-session -t a1
$TM list-sessions -F '#{session_name}' -f '#{m:a*,#{session_name}}'
