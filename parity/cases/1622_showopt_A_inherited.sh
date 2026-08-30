# show-options -A also prints options a window inherits from the global set
# (cmd-show-options.c:122, :231); without -A only options set on the window
# itself appear.
$TM set -g @inherited from-global
$TM setw -g monitor-activity off
$TM setw monitor-activity on
echo "== window scope, no -A =="
$TM show-options -w | grep -c 'monitor-activity on'
echo "== window scope, -A shows an inherited option too =="
$TM show-options -wA | grep -c 'aggressive-resize'
echo "== -A marks inherited values with a leading * =="
$TM show-options -wA | grep '^\*' | head -1 | perl -pe 's/^(\*[a-z-]+).*/$1 <value>/'
