# kill-session -f (filter, -a only) and -g validation.
$TM new-session -d -s alpha "sleep 300"
$TM new-session -d -s beta "sleep 300"
$TM kill-session -f '#{==:1,1}'
$TM kill-session -a -f '#{m:beta,#{session_name}}' -t alpha
$TM list-sessions -F '#{session_name}'
