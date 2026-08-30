# break-pane -b puts the new window before the current one instead of after it,
# and -a after it; both respect renumber-windows.
$TM set -g automatic-rename off
$TM set -g renumber-windows off
$TM new-window -d -n anchor
$TM select-window -t anchor
$TM split-window -d -t anchor
$TM break-pane -d -a -n after-it -s anchor.1; echo "-a rc=$?"
$TM list-windows -F '  #{window_index}:#{window_name}' | sort
$TM split-window -d -t anchor
$TM break-pane -d -b -n before-it -s anchor.1; echo "-b rc=$?"
$TM list-windows -F '  #{window_index}:#{window_name}' | sort
$TM set -gu renumber-windows
