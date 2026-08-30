# new-pane makes a FLOATING pane, sized half the window's width by a quarter of
# its height minus the two border columns/rows, and cascaded down-right from
# (4,2) by (+4,+2) per pane (layout.c:1678-1747). -x/-y give the size and -X/-Y
# the offset, each accepting a percentage of the window.
$TM set -g status off
geom() { $TM display-message -p -t "$1" '#{pane_width}x#{pane_height} at #{pane_left},#{pane_top}'; }
newest() { $TM list-panes -F '#{pane_id}' | tr -d '%' | sort -n | tail -1 | perl -pe 's/^/%/'; }
echo "window: $($TM display-message -p '#{window_width}x#{window_height}')"
$TM new-pane -d -E; echo "rc=$?"
a=$(newest); echo "first floating:  $(geom "$a")"
$TM new-pane -d -E; echo "rc=$?"
b=$(newest); echo "second floating: $(geom "$b")"
echo "both are floating: $($TM display-message -p -t "$a" '#{pane_floating_flag}')$($TM display-message -p -t "$b" '#{pane_floating_flag}')"
echo "== -x and -y give the size, the borders coming off the top =="
$TM new-pane -d -E -x 20 -y 8; echo "rc=$?"
echo "sized: $(geom "$(newest)")"
echo "== a percentage of the window works too =="
$TM new-pane -d -E -x 50% -y 50%; echo "rc=$?"
echo "sized: $(geom "$(newest)")"
echo "== -X and -Y place it =="
$TM new-pane -d -E -x 10 -y 4 -X 30 -Y 10; echo "rc=$?"
echo "placed: $(geom "$(newest)")"
echo "== a size the window cannot hold is refused =="
$TM new-pane -d -E -x 1 2>&1; echo "rc=$?"
$TM new-pane -d -E -y 1 2>&1; echo "rc=$?"
$TM new-pane -d -E -x notanumber 2>&1; echo "rc=$?"
echo "panes: $($TM list-panes | wc -l | tr -d ' ')"
