# Pane splits and their BORDERS, as an attached client actually paints them.
#
# Everything server-side about a split is a layout string: list-panes and
# #{window_layout} report x/y/width/height and every existing case stops there.
# The border cells between those rectangles are not in the layout at all --
# they are synthesised at draw time by screen_redraw_draw_borders/
# screen_redraw_cell_border, which only runs for an ATTACHED CLIENT. A port can
# compute a byte-identical layout string and still draw the wrong glyph at a
# junction, put the vertical rule one column off, or leave the border row blank.
#
# So this case builds a real client the way 1504/1507/1508 do -- a second server
# inside a pane of the first, with a client attached to it -- and reads the
# painted grid back with capture-pane on the OUTER server.
#
# What is pinned here:
#   * the vertical rule's COLUMN after `split-window -h` on an 80-wide window
#     (40 left / 1 border / 39 right), proved by where each pane's own text lands
#   * the horizontal rule after `split-window -v`, and that it spans only the
#     sub-rectangle it belongs to (a T, not a full-width line)
#   * the JUNCTION glyphs: screen_redraw_cell_border returns a different cell for
#     a left-tee, right-tee, top-tee, bottom-tee and cross, and getting the
#     bitmask wrong is invisible to every layout-string case
#   * zoom: resize-pane -Z must repaint the window with NO borders at all, and
#     unzoom must restore the exact pre-zoom border picture
#
# Determinism: no clock, no tty name, no pid is printed. Each pane runs
# `echo <marker>; sleep 300` so the pane's own origin is visible in the grid,
# and every wait is a poll on the real painted screen, never a blind sleep.
set -- $TM
BIN="$1"
ISOCK="spl_$$_inner"
I() { $BIN -L "$ISOCK" "$@"; }
# Belt and braces: if the harness timeout kills this script mid-way, the inner
# server would otherwise outlive the run.
trap '$BIN -L "$ISOCK" kill-server 2>/dev/null' EXIT INT TERM

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'echo P0; sleep 300'
# The status line is 1504's subject and carries a clock; drop it so all 24 rows
# belong to panes and borders.
I set -g status off
I set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering. tmux ignores the unknown user option.
I set -g @ztmux-ratatui off

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"

snap() { $TM capture-pane -p -e -t client | cksum; }

# Wait on the PAINTED screen, not on a timer: settle when two consecutive
# captures agree AND differ from the pre-command capture. Under suite load a
# blind sleep races the client's redraw.
wait_change() {
  local prev="$1" i=0 a b
  while [ $i -lt 60 ]; do
    a=$(snap); sleep 0.15; b=$(snap)
    if [ "$a" = "$b" ] && [ "$a" != "$prev" ]; then return 0; fi
    i=$((i+1)); sleep 0.05
  done
  echo "wait_change: TIMED OUT"; return 1
}

# Collapse runs of spaces and of box-drawing characters to <char>{xN}. The run
# lengths ARE the geometry, so nothing is lost, and a one-column border error
# still shows up as a changed count.
grid() {
  echo "== $1"
  $TM capture-pane -p -t client \
    | perl -CSD -pe 's/([ \x{2500}-\x{257f}])\1+/sprintf("%s{x%d}",$1,length($&))/ge; s/[ \t]+$//' \
    | uniq -c | perl -pe 's/^\s*(\d+)\s/[$1] /'
}

# The pre-attach screen is blank, so this first wait covers the client attaching
# AND its first paint as well as the split itself.
p=$(snap)
I split-window -h -t alpha:one 'echo P1; sleep 300'
wait_change "$p"
grid "split -h  (2 panes, one vertical rule)"

# A vertical split inside the RIGHT pane only: the horizontal rule must stop at
# the existing vertical rule and turn it into a left-tee.
p=$(snap); I split-window -v -t alpha:one.1 'echo P2; sleep 300'; wait_change "$p"
grid "split -v inside pane 1  (left-tee junction)"

# Now split the LEFT pane too, at the same row: the left-tee must become a cross
# and the right edge a right-tee.
p=$(snap); I split-window -v -t alpha:one.0 'echo P3; sleep 300'; wait_change "$p"
grid "split -v inside pane 0  (cross junction)"

# Zoom: the window is repainted as the single zoomed pane with no borders at all.
p=$(snap); I resize-pane -Z -t alpha:one.2; wait_change "$p"
grid "zoomed  (no borders)"
echo "zoom flags: $(I display-message -p -t alpha:one '#{window_zoomed_flag} #{window_panes}')"

# Unzoom must restore the identical border picture, junctions included.
p=$(snap); I resize-pane -Z -t alpha:one.2; wait_change "$p"
grid "unzoomed  (borders restored)"

# Closing the bottom-right pane collapses the right column back to one pane, so
# the cross must degrade to a plain vertical rule again.
p=$(snap); I kill-pane -t alpha:one.2; wait_change "$p"
grid "after kill-pane  (junction degrades)"

I kill-server 2>/dev/null
