# -P prints the new session (default format `#{session_name}:`), and -F chooses
# what is printed.
$TM new-session -d -P -s printed -x 80 -y 24
$TM new-session -d -P -F '#{session_name}/#{window_index}/#{window_name}' -s fmt -n win -x 80 -y 24
$TM new-session -d -P -F '#{session_windows}' -s counted -x 80 -y 24
