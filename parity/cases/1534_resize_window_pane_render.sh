# resize-window and resize-pane, checked against the pixels a client paints.
#
# Pane geometry is easy to get right in #{pane_width}/#{pane_height} and still
# wrong on screen: the numbers come out of layout_resize(), but the picture
# comes out of screen_redraw_screen() drawing the pane borders at the offsets
# that layout produced, and -- once the window is smaller than the tty -- the
# leftover columns and rows in the pane-border fill character. Server-side
# cases can only see the first half. This case pins both, side by side, for
# every kind of resize: a whole-window resize, a relative pane resize, an
# absolute pane resize, a vertical pane resize, and resize-window -A.
#
# The window is split into three panes (one vertical border, one horizontal
# border) so a single row-1 capture plus the border row numbers locate every
# edge, and shrinking the window to 60x20 inside an 80x24 tty deliberately
# exposes the fill region on two sides at once.
#
# Built like cases 1504/1507/1508/1510: a second server inside a pane of the
# first, with a real client attached, so capture-pane on the OUTER server reads
# back exactly what the inner client drew.
set -- $TM
BIN="$1"
ISOCK="rwp_$$_inner"

nl() { perl -ne 'chomp; s/\s+$//; printf "%2d|%s|\n", $., $_'; }

# Render signature for waiting: byte offsets of every vertical border (U+2502 =
# \xe2\x94\x82) on row 1, the number of pane-border fill cells (U+00B7 =
# \xc2\xb7) on row 1, and every row carrying a horizontal border run (U+2500 =
# \xe2\x94\x80). Together those pin both border axes and the fill region. The
# runner exports LC_ALL=C, so perl works on bytes.
sig() {
  $TM capture-pane -p -t "$1" | perl -ne '
    chomp; s/\s+$//;
    if ($. == 1) { while (/\xe2\x94\x82/g) { push @v, pos($_) } $d = () = /\xc2\xb7/g; }
    push @h, $. if /\xe2\x94\x80/;
    END { printf "%s:%d:%s", join(",", @v), $d || 0, join(",", @h) }'
}
# Never sleep blindly for a redraw -- under suite load the paint can land late.
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
panes() {
  $BIN -L "$ISOCK" list-panes -t alpha:one \
    -F '  pane #{pane_index}: #{pane_width}x#{pane_height} at #{pane_left},#{pane_top}'
}

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" split-window -d -h -t alpha:one 'sleep 300'
$BIN -L "$ISOCK" split-window -d -v -t alpha:one.0 'sleep 300'

$TM new-window -d -n cA "$BIN -L $ISOCK attach -t alpha"
wait_clients 1
wait_render cA '43:0:12'
echo "== attached, window fills the 80x24 tty"
echo "win=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{window_width}x#{window_height}')"
panes
$TM capture-pane -p -t cA | nl

# resize-window to something smaller than the tty. Everything outside the
# window becomes pane-border fill: 19 cells on the right of every row and four
# whole rows underneath.
$BIN -L "$ISOCK" set -g window-size manual
$BIN -L "$ISOCK" resize-window -t alpha:one -x 60 -y 20
wait_render cA '33,65:19:11,21'
echo "== resize-window -x 60 -y 20"
echo "win=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{window_width}x#{window_height}')"
panes
$TM capture-pane -p -t cA | nl

# Relative pane resize: the vertical border slides ten columns left and the two
# panes either side of it take up the difference.
$BIN -L "$ISOCK" resize-pane -t alpha:one.0 -L 10
wait_render cA '23,65:19:11,21'
echo "== resize-pane -t 0 -L 10"
panes
$TM capture-pane -p -t cA | nl

# Vertical pane resize by absolute height: the horizontal border jumps.
$BIN -L "$ISOCK" resize-pane -t alpha:one.1 -y 4
wait_render cA '23,65:19:16,21'
echo "== resize-pane -t 1 -y 4"
panes
$TM capture-pane -p -t cA | nl

# resize-window -A snaps the window back to the largest attached client, so the
# fill region disappears and the layout is stretched into the full tty.
$BIN -L "$ISOCK" resize-window -t alpha:one -A
wait_render cA '33:0:18'
echo "== resize-window -A (largest client)"
echo "win=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{window_width}x#{window_height}')"
panes
$TM capture-pane -p -t cA | nl

$BIN -L "$ISOCK" kill-server 2>/dev/null
