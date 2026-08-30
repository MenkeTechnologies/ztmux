# base-index and pane-base-index decide where numbering starts for windows and
# panes created afterwards; existing objects keep the indexes they were given.
$TM set -g automatic-rename off
echo "== defaults =="
$TM new-window -d -n before
$TM list-windows -F '#{window_index}:#{window_name}' | sort
$TM set -g base-index 5
$TM setw -g pane-base-index 3
$TM new-window -d -n after
$TM list-windows -F '#{window_index}:#{window_name}' | sort
echo "== panes in the new window =="
$TM split-window -d -t after
$TM list-panes -t after -F '#{pane_index}' | sort
echo "== and in the older one, which keeps its base =="
$TM split-window -d -t before
$TM list-panes -t before -F '#{pane_index}' | sort
$TM set -gu base-index; $TM setw -gu pane-base-index
