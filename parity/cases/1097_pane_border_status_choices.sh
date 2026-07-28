# pane-border-status: every choice value is accepted. The two *-floating values
# apply to floating panes only, so a window reads them back as themselves but
# renders tiled panes as if off.
for v in off top bottom top-floating bottom-floating; do
  $TM set -w pane-border-status "$v"
  $TM display-message -p "$v=#{pane-border-status}"
done
$TM set -w pane-border-status bogus
# Sizing accounts for a status line in the row the pane would otherwise use.
$TM set -w pane-border-status top
$TM split-window -v -d "sleep 300"
$TM resize-pane -y 8
$TM list-panes -F '#{pane_index} #{pane_height}@#{pane_y}'
