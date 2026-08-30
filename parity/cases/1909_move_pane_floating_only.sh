# In next-3.7 move-pane is the floating-pane command: its target must be a
# floating pane and a tiled one is refused (cmd-join-pane.c:46-50). The -P form
# places that floating pane and -z restacks it.
#
# The floating pane is made with `break-pane -W`, not `split-window -W`: on the
# vendored reference the latter never returns (the server keeps serving, that
# client hangs), which is also why case 1729 uses break-pane and says so.
$TM set -g automatic-rename off
$TM split-window -d
echo "== a tiled target is refused =="
$TM move-pane -s 0 -t 1 2>&1; echo "rc=$?"
$TM move-pane -P centre -t 1 2>&1; echo "rc=$?"
echo "== a floating pane accepts them =="
$TM break-pane -d -W -n floated -x 20 -y 5 -X 4 -Y 2
$TM list-panes -t floated -F '  #{pane_index} floating=#{pane_floating_flag} at #{pane_left},#{pane_top}' | sort
$TM move-pane -P top-left -t floated.0; echo "-P top-left rc=$?"
$TM list-panes -t floated -F '  #{pane_index} at #{pane_left},#{pane_top}' | sort
$TM move-pane -P bottom-right -t floated.0; echo "-P bottom-right rc=$?"
$TM list-panes -t floated -F '  #{pane_index} at #{pane_left},#{pane_top}' | sort
$TM move-pane -z 1 -t floated.0 2>&1; echo "-z rc=$?"
echo "== an unknown position =="
$TM move-pane -P nonsense -t floated.0 2>&1; echo "rc=$?"
