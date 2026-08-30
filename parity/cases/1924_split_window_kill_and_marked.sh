# split-window -k kills the target pane and takes its place, and -m marks the
# new pane as it is created.
$TM set -g automatic-rename off
$TM new-window -d -n sp 'sleep 300'
$TM split-window -d -t sp 'sleep 300'
$TM list-panes -t sp -F '  #{pane_index}=#{pane_id}' | sort
echo "== -k replaces the target =="
$TM split-window -d -k -t sp.1 'sleep 300'; echo "rc=$?"
$TM list-panes -t sp -F '  #{pane_index}=#{pane_id}' | sort
echo "== -m marks the new pane =="
$TM split-window -d -m -t sp.0 'sleep 300'; echo "rc=$?"
$TM list-panes -t sp -F '  #{pane_index} marked=#{pane_marked}' | sort
echo "marked pane: $($TM display-message -p -t '{marked}' '#{pane_index}' 2>&1)"
