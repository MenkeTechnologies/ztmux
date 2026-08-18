# The four mode-tree screens as a CLIENT paints them.
#
# choose-tree (prefix w), choose-client (prefix D), choose-buffer (prefix =) and
# choose-tree's session view (prefix s) all render through mode_tree_draw. None
# of it is reachable from the server side: mode_tree_draw only runs for an
# attached client, so before this case the entire row composition was uncovered
# and had silently drifted to an older revision -- no MODE_TREE_PREFIX_FORMAT,
# no per-depth alignment, and a hand-composed row string instead of the C's
# prefix/text split through format_draw.
#
# What that cost, all of it invisible to every server-side case: the green +
# expander, the red - collapse marker, the themelightgrey row colour, the
# "#[fg=themelightgrey]: #[default]" separator, and the per-mode default format
# strings (window-tree/window-client/window-customize had each drifted too).
#
# Built like cases 1504 and 1507: a second server inside a pane of the first,
# with a client attached, so capture-pane on the OUTER server reads back exactly
# what the inner client drew.
set -- $TM
BIN="$1"
ISOCK="mtr_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" new-window -d -t alpha -n two 'sleep 300'
$BIN -L "$ISOCK" new-session -d -s beta -n solo -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" set-buffer -b b1 hello
$BIN -L "$ISOCK" set-buffer -b b2 world


# Wait for the inner client's active pane to actually enter (or leave) a mode
# rather than sleeping a fixed amount: this case runs alongside the rest of the
# suite, and under CPU contention a blind sleep races the client's first draw.

# Determinism: choose-client names the client's tty, and both choose-client and
# choose-buffer carry a clock. The pty number is whatever the OS hands out and
# the clock can cross a minute boundary between the two runs, so mask both.
scrub() { perl -pe 's{/dev/tty[a-z0-9]+}{/dev/ttyDEV}g; s/\d\d:\d\d/HH:MM/g'; }

wait_mode() {   # $1 = expected mode name, or "" for no mode
  local want="$1" i=0 got
  while [ $i -lt 100 ]; do
    got=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{pane_mode}' 2>/dev/null)
    [ "$got" = "$want" ] && { sleep 0.4; return 0; }
    i=$((i+1)); sleep 0.1
  done
  echo "wait_mode: timed out waiting for [$want], last=[$got]"
  return 1
}

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
sleep 2

# choose-tree: two sessions, one with two windows, so the row set exercises
# depth 0 and 1, the expander, the branch glyphs and the last-child marker.
$TM send-keys -t client C-b; sleep 0.5
$TM send-keys -t client w
wait_mode tree-mode
echo "choose-tree:"
$TM capture-pane -p -e -t client | sed -n '1,6p' | cat -v | scrub
$TM send-keys -t client q
wait_mode ''

# choose-buffer: a flat list, so it pins the no-branch path.
$TM send-keys -t client C-b; sleep 0.5
$TM send-keys -t client '='
wait_mode buffer-mode
echo "choose-buffer:"
$TM capture-pane -p -e -t client | sed -n '1,4p' | cat -v | scrub
$TM send-keys -t client q
wait_mode ''

# choose-client. Its row carries the client tty and client_activity; scrub both.
$TM send-keys -t client C-b; sleep 0.5
$TM send-keys -t client D
wait_mode client-mode
echo "choose-client:"
$TM capture-pane -p -e -t client | sed -n '1,3p' | cat -v | scrub
$TM send-keys -t client q
wait_mode ''

$BIN -L "$ISOCK" kill-server 2>/dev/null
