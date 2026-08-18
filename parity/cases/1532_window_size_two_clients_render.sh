# window-size largest/smallest/latest with TWO clients of DIFFERENT sizes,
# as each client actually paints it.
#
# Two attached clients is the only situation in which window-size means
# anything, and every part of the answer is invisible from the server side:
# default_window_size() picks the number, but what the user SEES is
# screen_redraw_screen() reconciling a window that is bigger or smaller than
# the tty it is drawn on. A client LARGER than the window gets the window
# drawn into the top-left corner with the leftover rows painted in the
# pane-border fill character; a client SMALLER than the window gets the window
# clipped. Nothing else in the suite attaches two differently sized clients at
# once, so both of those redraw paths were completely uncovered.
#
# Splitting the inner window puts a horizontal pane border in the middle of the
# screen, so the negotiated height is readable straight off the rendered row
# numbers and not only off #{window_height}.
#
# Built like cases 1504/1507/1508/1510: a second server inside panes of the
# first, with real clients attached, so capture-pane on the OUTER server reads
# back exactly what the inner clients drew. Client A gets a full 80x24 outer
# window; client B gets an 80x15 outer pane, so the two ttys genuinely differ.
set -- $TM
BIN="$1"
ISOCK="wstc_$$_inner"

# Row-numbered, trailing-space-stripped screen dump. The row numbers are the
# assertion: the whole point is WHERE the border and the fill land.
nl() { perl -ne 'chomp; s/\s+$//; printf "%2d|%s|\n", $., $_'; }

# Compact render signature, used only for waiting: which rows carry a
# box-drawing glyph (bytes \xe2\x94 / \xe2\x95 = U+2500..U+257F), how many rows
# are pane-border fill (\xc2\xb7 = U+00B7 MIDDLE DOT), and the text of row 1.
# The runner exports LC_ALL=C, so perl sees bytes rather than characters.
sig() {
  $TM capture-pane -p -t "$1" | perl -ne '
    chomp; s/\s+$//;
    push @b, $. if /\xe2[\x94\x95]/;
    $f++ if /\xc2\xb7/;
    $r1 = $_ if $. == 1;
    END { printf "%s:%d:%s", join(",", @b), $f || 0, $r1 }'
}

# Never sleep blindly waiting for a redraw: under suite load the first paint
# can land hundreds of milliseconds late. Poll the real rendered state. A
# timeout prints a diagnostic, which diverges and fails the case -- intended.
wait_render() {  # $1 = outer target, $2 = expected signature
  local i=0 got=
  while [ $i -lt 200 ]; do
    got=$(sig "$1")
    [ "$got" = "$2" ] && return 0
    i=$((i+1)); sleep 0.05
  done
  echo "wait_render $1: timed out want=[$2] got=[$got]"
}
wait_clients() {  # $1 = expected client count
  local i=0 got=
  while [ $i -lt 200 ]; do
    got=$($BIN -L "$ISOCK" list-clients -F x 2>/dev/null | wc -l | tr -d ' ')
    [ "$got" = "$1" ] && return 0
    i=$((i+1)); sleep 0.05
  done
  echo "wait_clients: timed out want=[$1] got=[$got]"
}
wait_size() {  # $1 = expected WxH of alpha:one
  local i=0 got=
  while [ $i -lt 200 ]; do
    got=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{window_width}x#{window_height}' 2>/dev/null)
    [ "$got" = "$1" ] && return 0
    i=$((i+1)); sleep 0.05
  done
  echo "wait_size: timed out want=[$1] got=[$got]"
}

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" set -g window-size largest
$BIN -L "$ISOCK" split-window -d -v -t alpha:one 'sleep 300'

# Client A first, and wait until it is the ONLY client before starting B, so
# "which client attached most recently" is a fact rather than a race --
# window-size latest depends on exactly that.
$TM new-window -d -n cA "$BIN -L $ISOCK attach -t alpha"
wait_clients 1
# Client B lives in an outer pane shrunk to 15 rows, so its tty is 80x15.
$TM new-window -d -n cB 'sleep 300'
$TM split-window -d -t cB 'sleep 300'
$TM resize-pane -t cB.0 -y 8
$TM respawn-pane -k -t cB.1 "$BIN -L $ISOCK attach -t alpha"
wait_clients 2

echo "clients:"
$BIN -L "$ISOCK" list-clients -F '#{client_width}x#{client_height}' | sort

# largest: 80x23, client A's tty minus its status line. Client B only has 14
# rows of window to show, so the lower pane is clipped away entirely.
$BIN -L "$ISOCK" set -g window-size largest
wait_size 80x23
wait_render cA '12:0:'
wait_render cB.1 '12:0:'
echo "== largest: win=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{window_width}x#{window_height}')"
echo "-- A(80x24):"; $TM capture-pane -p -t cA | nl
echo "-- B(80x15):"; $TM capture-pane -p -t cB.1 | nl

# smallest: 80x14. Client A is now BIGGER than the window, so row 15 is the
# window's bottom edge and rows 16-23 are pane-border fill.
$BIN -L "$ISOCK" set -g window-size smallest
wait_size 80x14
wait_render cA '7,15:8:'
wait_render cB.1 '7:0:'
echo "== smallest: win=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{window_width}x#{window_height}')"
echo "-- A(80x24):"; $TM capture-pane -p -t cA | nl
echo "-- B(80x15):"; $TM capture-pane -p -t cB.1 | nl

# latest: the most recently used client. B attached last and nothing has been
# typed at A, so the window follows B and keeps the 80x14 shape.
$BIN -L "$ISOCK" set -g window-size latest
wait_size 80x14
wait_render cA '7,15:8:'
echo "== latest (B most recent): win=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{window_width}x#{window_height}')"
echo "-- A(80x24):"; $TM capture-pane -p -t cA | nl

# One keystroke at A makes A the most recently used client, and under `latest`
# the window has to follow it: back to 80x23 and A's fill rows disappear.
# Escape is inert for the inner `sleep 300`.
$TM send-keys -t cA Escape
wait_size 80x23
# The signature's third field is row 1, so this also waits for the tty echo of
# the Escape ("^[", two printable characters) to be drawn. Without that the
# capture below would race the echo: row 1 empty on one run, "^[" on the next.
wait_render cA '12:0:^['
wait_render cB.1 '12:0:^['
echo "== latest (A most recent): win=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{window_width}x#{window_height}')"
echo "-- A(80x24):"; $TM capture-pane -p -t cA | nl
echo "-- B(80x15):"; $TM capture-pane -p -t cB.1 | nl

$BIN -L "$ISOCK" kill-server 2>/dev/null
