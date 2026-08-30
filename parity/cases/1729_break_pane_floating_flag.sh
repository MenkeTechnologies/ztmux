# break-pane -W breaks the pane out as a floating pane, with -x/-y sizing it and
# -X/-Y placing it (`cmd-break-pane.c:37`).
#
# The sibling `split-window -W` is deliberately not exercised: on the vendored
# next-3.7 reference that command prints its -P line and then never exits (the
# server keeps serving; only that client hangs), so there is nothing stable to
# compare against.
$TM set -g automatic-rename off
$TM split-window -d
$TM break-pane -d -W -n floated -x 20 -y 5 -X 5 -Y 2 -P -F '#{window_name}:#{pane_floating_flag}'; echo "rc=$?"
$TM list-panes -t floated -F '#{pane_index} floating=#{pane_floating_flag} #{pane_left},#{pane_top} #{pane_width}x#{pane_height}' | sort
echo "== and a plain break-pane is not floating =="
$TM split-window -d
$TM break-pane -d -n tiled -P -F '#{window_name}:#{pane_floating_flag}'
$TM list-panes -t tiled -F '#{pane_index} floating=#{pane_floating_flag}' | sort
