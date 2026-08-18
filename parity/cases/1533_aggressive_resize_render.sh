# aggressive-resize, as the affected client actually repaints.
#
# aggressive-resize changes WHICH sessions vote on a window's size: off, every
# session the window is linked into counts; on, only the sessions where it is
# the CURRENT window count (window.c, recalculate_size / RESIZE_ALL). The
# option therefore only does anything when one window is linked into two
# sessions that are looking at different things -- a shape no server-only case
# in the suite builds -- and the visible consequence is a full redraw of every
# client showing that window.
#
# Setup: window "shared" (split in two so a pane border marks the height) lives
# in session alpha AND is link-window'd into beta as beta:1, while beta stays
# on its own window "solo". Client A (80x24 tty) is on alpha, client B (80x15
# tty) is on beta. With window-size smallest and aggressive-resize off, beta
# drags "shared" down to 14 rows even though nobody is looking at it there, and
# client A ends up painting eight rows of pane-border fill under the window.
# Turning aggressive-resize on drops beta's vote and A gets its full 23 rows.
# Pointing beta AT the shared window puts the vote back and shrinks it again.
#
# Built like cases 1504/1507/1508/1510: a second server inside panes of the
# first, with real clients attached, so capture-pane on the OUTER server reads
# back exactly what the inner clients drew.
set -- $TM
BIN="$1"
ISOCK="aggr_$$_inner"

nl() { perl -ne 'chomp; s/\s+$//; printf "%2d|%s|\n", $., $_'; }

# Render signature for waiting: rows carrying a box-drawing glyph (bytes
# \xe2\x94 / \xe2\x95 = U+2500..U+257F) and the count of pane-border fill rows
# (\xc2\xb7 = U+00B7). The runner exports LC_ALL=C so perl sees bytes.
sig() {
  $TM capture-pane -p -t "$1" | perl -ne '
    chomp; s/\s+$//;
    push @b, $. if /\xe2[\x94\x95]/;
    $f++ if /\xc2\xb7/;
    END { printf "%s:%d", join(",", @b), $f || 0 }'
}
wait_render() {  # $1 = outer target, $2 = expected signature
  local i=0 got=
  while [ $i -lt 200 ]; do
    got=$(sig "$1")
    [ "$got" = "$2" ] && return 0
    i=$((i+1)); sleep 0.05
  done
  echo "wait_render $1: timed out want=[$2] got=[$got]"
}
wait_clients() {
  local i=0 got=
  while [ $i -lt 200 ]; do
    got=$($BIN -L "$ISOCK" list-clients -F x 2>/dev/null | wc -l | tr -d ' ')
    [ "$got" = "$1" ] && return 0
    i=$((i+1)); sleep 0.05
  done
  echo "wait_clients: timed out want=[$1] got=[$got]"
}
wait_size() {  # $1 = target, $2 = expected WxH
  local i=0 got=
  while [ $i -lt 200 ]; do
    got=$($BIN -L "$ISOCK" display-message -p -t "$1" '#{window_width}x#{window_height}' 2>/dev/null)
    [ "$got" = "$2" ] && return 0
    i=$((i+1)); sleep 0.05
  done
  echo "wait_size $1: timed out want=[$2] got=[$got]"
}

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n shared -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" set -g window-size smallest
$BIN -L "$ISOCK" split-window -d -v -t alpha:shared 'sleep 300'
$BIN -L "$ISOCK" new-session -d -s beta -n solo -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" link-window -s alpha:0 -t beta:1
$BIN -L "$ISOCK" select-window -t beta:0

$TM new-window -d -n cA "$BIN -L $ISOCK attach -t alpha"
wait_clients 1
$TM new-window -d -n cB 'sleep 300'
$TM split-window -d -t cB 'sleep 300'
$TM resize-pane -t cB.0 -y 8
$TM respawn-pane -k -t cB.1 "$BIN -L $ISOCK attach -t beta"
wait_clients 2

echo "clients:"
$BIN -L "$ISOCK" list-clients -F '#{client_width}x#{client_height} #{client_session}:#{window_name}' | sort

# off: beta votes even though it is showing "solo", so "shared" is squeezed to
# beta's client height and A paints fill rows below it.
$BIN -L "$ISOCK" setw -g aggressive-resize off
wait_size alpha:shared 80x14
wait_render cA '7,15:8'
echo "== aggressive-resize off"
$BIN -L "$ISOCK" list-windows -a -F '#{session_name}:#{window_name} #{window_width}x#{window_height}'
echo "-- A on alpha:shared (80x24 tty):"; $TM capture-pane -p -t cA | nl

# on: beta is not showing "shared", so its vote is dropped and the window grows
# to client A's full height. No fill rows left.
$BIN -L "$ISOCK" setw -g aggressive-resize on
wait_size alpha:shared 80x23
wait_render cA '12:0'
echo "== aggressive-resize on"
$BIN -L "$ISOCK" list-windows -a -F '#{session_name}:#{window_name} #{window_width}x#{window_height}'
echo "-- A on alpha:shared (80x24 tty):"; $TM capture-pane -p -t cA | nl

# Still on, but now beta is pointed at the shared window, so it votes again and
# the window shrinks back -- and client B, which has just switched to it, has to
# paint the same window on its shorter tty.
$BIN -L "$ISOCK" select-window -t beta:1
wait_size alpha:shared 80x14
wait_render cA '7,15:8'
wait_render cB.1 '7:0'
echo "== aggressive-resize on, beta now viewing the shared window"
$BIN -L "$ISOCK" list-windows -a -F '#{session_name}:#{window_name} #{window_width}x#{window_height}'
echo "-- A on alpha:shared (80x24 tty):"; $TM capture-pane -p -t cA | nl
echo "-- B on beta:shared (80x15 tty):"; $TM capture-pane -p -t cB.1 | nl

$BIN -L "$ISOCK" kill-server 2>/dev/null
