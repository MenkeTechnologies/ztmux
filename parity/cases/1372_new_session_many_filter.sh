$TM new-session -d -s w1 -x 80 -y 24
$TM new-session -d -s w2 -x 80 -y 24
$TM new-session -d -s w3 -x 80 -y 24
$TM list-sessions -F '#{session_name}' -f '#{m:w?,#{session_name}}'
