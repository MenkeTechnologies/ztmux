# -p sets a pane option; the pane value overrides the window one for that pane
# only, and -pu removes it again.
$TM split-window -d
$TM setw -g window-status-separator '|'
$TM set -p @paneopt one
$TM list-panes -F '#{pane_index} [#{@paneopt}]'
$TM set -pu @paneopt
$TM list-panes -F '#{pane_index} [#{@paneopt}]'
echo "== a window option asked for in the pane set =="
$TM show-options -pv window-status-separator 2>&1; echo "rc=$?"
