# swap-window exchanges two windows' indexes; -d leaves the current window
# unchanged, without -d the current window follows the source.
$TM set -g automatic-rename off
$TM new-window -d -n one
$TM new-window -d -n two
$TM select-window -t one
$TM list-windows -F '#{window_index}:#{window_name}:#{window_active}' | sort
$TM swap-window -d -s one -t two; echo "-d rc=$?"
$TM list-windows -F '#{window_index}:#{window_name}:#{window_active}' | sort
$TM swap-window -s one -t two; echo "plain rc=$?"
$TM list-windows -F '#{window_index}:#{window_name}:#{window_active}' | sort
