# A zoomed pane across a window resize, a detach and a reattach -- as painted.
#
# Zoom is a layout that exists only while the window is drawn: window_zoom()
# swaps the layout tree for a single full-window cell, and everything a user
# can check about it (the pane filling the window, the "Z" flag in the status
# line, the borders coming back on unzoom) is produced by the client redraw
# path. A resize while zoomed has to resize the ZOOMED layout and keep the
# saved layout for later, and a detach/reattach cycle has to bring back both
# the zoom flag and the pane's scrollback untouched. None of that is reachable
# from a server-only case.
#
# The first pane prints two marker lines and then sleeps, so "the content
# survived" is an assertion about actual screen cells and not just about
# #{pane_id} still existing. window-size is manual, so the client tty (80x24)
# stays deliberately larger than the window after the resize to 70x18: the
# pane-border fill on the right and bottom then proves the redraw is clipping
# to the window and not to the terminal.
#
# Built like cases 1504/1507/1508/1510: a second server inside a pane of the
# first, with a real client attached, so capture-pane on the OUTER server reads
# back exactly what the inner client drew.
set -- $TM
BIN="$1"
ISOCK="zrr_$$_inner"

nl() { perl -ne 'chomp; s/\s+$//; printf "%2d|%s|\n", $., $_'; }

# Render signature for waiting: byte offsets of the vertical borders (U+2502 =
# \xe2\x94\x82) on row 1, the pane-border fill cells (U+00B7 = \xc2\xb7) on row
# 1, the rows carrying a horizontal border run (U+2500 = \xe2\x94\x80), and the
# last row, which is the status line and therefore carries the zoom flag.
# LC_ALL=C is exported by the runner, so perl works on bytes.
sig() {
  $TM capture-pane -p -t "$1" | perl -ne '
    chomp; s/\s+$//;
    if ($. == 1) { while (/\xe2\x94\x82/g) { push @v, pos($_) } $d = () = /\xc2\xb7/g; }
    push @h, $. if /\xe2\x94\x80/;
    $l = $_;
    END { printf "%s:%d:%s:%s", join(",", @v), $d || 0, join(",", @h), $l }'
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
state() {
  $BIN -L "$ISOCK" display-message -p -t alpha:one \
    'win=#{window_width}x#{window_height} zoomed=#{window_zoomed_flag} p0=#{pane_width}x#{pane_height}'
}

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 \
  "sh -c 'printf \"ALPHA-ROW-1\nALPHA-ROW-2\n\"; sleep 300'"
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
# Manual, so neither the resize below nor the reattach is undone by the client.
$BIN -L "$ISOCK" set -g window-size manual
$BIN -L "$ISOCK" split-window -d -h -t alpha:one 'sleep 300'

$TM new-window -d -n cA "$BIN -L $ISOCK attach -t alpha"
wait_clients 1
wait_render cA '43:0::[alpha] 0:one*'
echo "== attached, two panes: $(state)"
$TM capture-pane -p -t cA | nl

# Zoom: one pane over the whole window, no borders at all, "Z" in the status.
$BIN -L "$ISOCK" resize-pane -Z -t alpha:one.0
wait_render cA ':0::[alpha] 0:one*Z'
echo "== zoomed: $(state)"
$TM capture-pane -p -t cA | nl

# Resize the window while it is zoomed. The zoom must survive, the zoomed pane
# must take the new window size, and the 10 columns / 5 rows of tty that the
# window no longer covers must become pane-border fill.
$BIN -L "$ISOCK" resize-window -t alpha:one -x 70 -y 18
wait_render cA '73:9:19:[alpha] 0:one*Z'
echo "== resize-window -x 70 -y 18 while zoomed: $(state)"
$TM capture-pane -p -t cA | nl

# Detach by killing the process that is running `attach`, then confirm the
# server kept the zoom and the manual size with nobody watching.
$TM respawn-pane -k -t cA 'sleep 300'
wait_clients 0
echo "== detached: $(state) clients=$($BIN -L "$ISOCK" list-clients -F x | wc -l | tr -d ' ')"

# Reattach on a fresh tty of the same size: the zoom, the geometry and the two
# marker lines in the pane must all come back exactly as they were.
$TM respawn-pane -k -t cA "$BIN -L $ISOCK attach -t alpha"
wait_clients 1
wait_render cA '73:9:19:[alpha] 0:one*Z'
echo "== reattached: $(state)"
$TM capture-pane -p -t cA | nl

# Unzoom restores the saved layout at the NEW window size, not the old one:
# the vertical border lands inside 70 columns, and the fill region is unchanged.
$BIN -L "$ISOCK" resize-pane -Z -t alpha:one.0
wait_render cA '38,75:9:19:[alpha] 0:one*'
echo "== unzoomed: $(state)"
$BIN -L "$ISOCK" list-panes -t alpha:one \
  -F '  pane #{pane_index}: #{pane_width}x#{pane_height} at #{pane_left},#{pane_top}'
$TM capture-pane -p -t cA | nl

$BIN -L "$ISOCK" kill-server 2>/dev/null
