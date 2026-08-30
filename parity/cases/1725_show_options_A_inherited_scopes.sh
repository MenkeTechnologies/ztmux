# -A adds the options a scope inherits rather than only those set on it, and
# marks the inherited ones with a leading *.
$TM new-window -d -n inh
$TM setw -t inh monitor-activity on
echo "== window scope without -A =="
$TM show-options -w -t inh | wc -l | tr -d ' '
echo "== window scope with -A is a superset =="
$TM show-options -wA -t inh | wc -l | tr -d ' '
echo "== the option set on the window is not marked =="
$TM show-options -wA -t inh | grep '^monitor-activity'
echo "== an inherited one is =="
$TM show-options -wA -t inh | grep '^\*aggressive-resize'
echo "== pane scope inherits from the window =="
$TM show-options -pA -t inh.0 | grep -c '^\*'
