$TM new-session -d -s doomed -x 80 -y 24
$TM kill-session -t doomed
$TM list-sessions -F '#{session_name}'
