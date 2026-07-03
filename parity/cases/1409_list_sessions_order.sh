# list-sessions -O sort order + -r reverse (wires the sort subsystem).
$TM new-session -d -s alpha "sleep 300"
$TM new-session -d -s bravo "sleep 300"
$TM list-sessions -O name -F '#{session_name}'
$TM list-sessions -O name -r -F '#{session_name}'
$TM list-sessions -O index -F '#{session_name}'
$TM list-sessions -O bogus
