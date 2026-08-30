# next-layout and previous-layout cycle the preset layouts and take a target;
# select-layout -n and -p are their aliases.
$TM set -g automatic-rename off
$TM new-window -d -n lay
$TM split-window -d -t lay
$TM split-window -d -t lay
shape() { $TM list-panes -t lay -F '#{pane_left},#{pane_top}' | sort | tr '\n' ' '; }
$TM select-layout -t lay even-horizontal >/dev/null
echo "even-horizontal: $(shape)"
$TM next-layout -t lay; echo "next-layout rc=$?"
echo "after next:      $(shape)"
$TM select-layout -t lay -n >/dev/null; echo "select-layout -n rc=$?"
echo "after -n:        $(shape)"
$TM previous-layout -t lay; echo "previous-layout rc=$?"
echo "after previous:  $(shape)"
$TM select-layout -t lay -p >/dev/null; echo "select-layout -p rc=$?"
echo "after -p:        $(shape)"
