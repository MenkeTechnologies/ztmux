$TM set-option -g monitor-bell on
$TM new-window -d -n w1 "printf '\a'; sleep 60"
$TM run-shell 'sleep 0.3'
$TM display-message -p '#{session_alert}'
