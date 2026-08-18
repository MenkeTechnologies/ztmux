# choose-tree's `i` information view, and the relative-time format it needs.
#
# `i` inside choose-tree (prefix w) swaps the preview panel for a per-item
# information panel: the pane table, then the window table, then the session
# table, separated by a horizontal rule with a vertical rule joining them down
# column 14 (window-tree.c:803 window_tree_draw_info). Which tables appear
# depends on the item type, so this case checks a window row AND a pane row.
#
# Two things had to land for this to match. window_tree_draw_info and its three
# info-line tables were unported, so `i` did nothing at all. And the tables use
# `#{t/r:...}` — relative age, e.g. "4s" — which the port expanded as an absolute
# timestamp because format_relative_time (format.c:4092) and the FORMAT_RELATIVE
# modifier were missing; that is checked directly below too, since a format
# modifier silently falling back to a different rendering is the kind of thing
# only an end-to-end screen comparison would otherwise catch.
#
# Clock fields are masked: the panel shows creation/activity times to the second
# and a relative age that ticks while the case runs.
set -- $TM
BIN="$1"
ISOCK="tiv_$$_inner"

scrub() {
  perl -pe 's{/dev/tty[a-z0-9]+}{/dev/ttyDEV}g;
            s/\d\d:\d\d:\d\d/HH:MM:SS/g;
            s/\(\d+[smhd][0-9smhd]*\)/(REL)/g;
            s/PID \d+/PID N/g;
            s/\s+$//'
}
wait_mode() {
  local want="$1" i=0 got
  while [ $i -lt 100 ]; do
    got=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{pane_mode}' 2>/dev/null)
    [ "$got" = "$want" ] && { sleep 0.4; return 0; }
    i=$((i+1)); sleep 0.1
  done
  echo "wait_mode: timed out waiting for [$want], last=[$got]"
}

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" new-window -d -t alpha -n two 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
$BIN -L "$ISOCK" set -g @ztmux-ratatui off

# The relative-time modifier on its own, before any drawing depends on it.
echo "t/r shape:"
$BIN -L "$ISOCK" display-message -p '#{t/r:session_created}' | perl -pe 's/^\d+[smhd][0-9smhd]*$/RELATIVE-SHAPE-OK/'
echo "t/p shape:"
$BIN -L "$ISOCK" display-message -p '#{t/p:session_created}' | perl -pe 's/^\d\d:\d\d$/PRETTY-SHAPE-OK/'

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
sleep 2

$TM send-keys -t client C-b; sleep 0.5
$TM send-keys -t client w
wait_mode tree-mode

# Window row: the window and session tables, no pane table.
$TM send-keys -t client i; sleep 1
echo "info view, window row:"
$TM capture-pane -p -t client | sed -n '1,20p' | scrub

# Toggling back restores the preview, and the box title follows.
$TM send-keys -t client i; sleep 1
echo "back to preview (box title):"
$TM capture-pane -p -t client | grep -o 'sort: [^)]*)[^M]*' | head -1

# Descend to a pane row: the pane table appears above the window one.
$TM send-keys -t client Down; sleep 0.4
$TM send-keys -t client Down; sleep 0.4
$TM send-keys -t client i; sleep 1
echo "info view, deeper row:"
$TM capture-pane -p -t client | sed -n '1,20p' | scrub

$TM send-keys -t client q
wait_mode ''
$BIN -L "$ISOCK" kill-server 2>/dev/null
