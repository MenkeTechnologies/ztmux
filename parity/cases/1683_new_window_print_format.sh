# -P prints the new window and -F chooses the format; -a and -b place it after or
# before the target index.
$TM set -g automatic-rename off
$TM new-window -d -P -n printed
$TM new-window -d -P -F '#{window_index}/#{window_name}/#{window_panes}' -n fmt
$TM new-window -d -t 1 -n anchor
$TM new-window -d -a -t anchor -P -F '#{window_index}' -n after
$TM new-window -d -b -t anchor -P -F '#{window_index}' -n before
$TM list-windows -F '#{window_index}:#{window_name}' | sort
