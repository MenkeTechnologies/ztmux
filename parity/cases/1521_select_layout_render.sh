# select-layout, as an attached client actually paints it.
#
# layout_set_even_h/even_v/main_h/main_v/tiled are exercised server-side today
# only through #{window_layout}, which is a checksummed list of rectangles. That
# string can be right while the SCREEN is wrong: the borders between those
# rectangles are synthesised at draw time by screen_redraw_draw_borders, which
# runs for an attached client and nowhere else. Rounding a pane one column short,
# drawing an even-vertical divider full width when it should stop at a column
# boundary, or mis-deriving main-pane-width from the option are all invisible to
# a layout-string comparison and all visible here.
#
# The client is built the way 1504/1507/1508 build one: a second server inside a
# pane of the first, with a client attached, so capture-pane on the OUTER server
# reads back exactly what the inner client drew.
#
# Pinned here, for a fixed 4-pane window on an exactly 80x24 screen:
#   * even-horizontal   -- four columns, and where the 80-3 columns of body go
#                          (the uneven remainder is the interesting part)
#   * even-vertical     -- three full-width rules at the right rows
#   * main-vertical     -- with main-pane-width pinned, so the case measures the
#                          layout and not the default
#   * main-horizontal   -- with main-pane-height pinned, plus the top-tee
#                          junctions where the stacked panes meet the main rule
#   * tiled             -- the 2x2 grid and its cross junction
#   * select-layout -E  -- spread-out, which re-runs the layout in place
#
# Determinism: no clock, no tty, no pid. Panes carry a fixed marker so their
# origins are visible, and every wait polls the painted screen.
set -- $TM
BIN="$1"
ISOCK="lay_$$_inner"
I() { $BIN -L "$ISOCK" "$@"; }
# Belt and braces: if the harness timeout kills this script mid-way, the inner
# server would otherwise outlive the run.
trap '$BIN -L "$ISOCK" kill-server 2>/dev/null' EXIT INT TERM

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'echo A; sleep 300'
I set -g status off
I set -g status-interval 0
I set -g @ztmux-ratatui off
# Pin both main-pane sizes: the defaults (80 wide / 24 high) are as large as the
# whole screen, which degenerates the main-* layouts to a 1-cell side strip and
# would hide any arithmetic error.
I set -g main-pane-width 34
I set -g main-pane-height 9

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"

snap() { $TM capture-pane -p -e -t client | cksum; }

wait_change() {
  local prev="$1" i=0 a b
  while [ $i -lt 60 ]; do
    a=$(snap); sleep 0.15; b=$(snap)
    if [ "$a" = "$b" ] && [ "$a" != "$prev" ]; then return 0; fi
    i=$((i+1)); sleep 0.05
  done
  echo "wait_change: TIMED OUT"; return 1
}

# Runs of spaces and of box-drawing glyphs collapse to <char>{xN}; the counts are
# the geometry, so a one-column layout error still changes the output.
grid() {
  echo "== $1"
  $TM capture-pane -p -t client \
    | perl -CSD -pe 's/([ \x{2500}-\x{257f}])\1+/sprintf("%s{x%d}",$1,length($&))/ge; s/[ \t]+$//' \
    | uniq -c | perl -pe 's/^\s*(\d+)\s/[$1] /'
}

# Build four panes. The screen is blank until the client attaches and paints, so
# this first wait covers attach + first paint as well.
p=$(snap)
I split-window -h -t alpha:one 'echo B; sleep 300'
wait_change "$p"
p=$(snap); I split-window -v -t alpha:one.1 'echo C; sleep 300'; wait_change "$p"
p=$(snap); I split-window -v -t alpha:one.0 'echo D; sleep 300'; wait_change "$p"
grid "4 panes before select-layout"

for L in even-horizontal even-vertical main-vertical main-horizontal tiled; do
  p=$(snap); I select-layout -t alpha:one "$L"; wait_change "$p"
  grid "select-layout $L"
done

# -E spreads panes out evenly inside the current layout; from tiled it is a
# distinct repaint, so the wait still has a real edge to detect.
p=$(snap); I select-layout -t alpha:one even-horizontal; wait_change "$p"
p=$(snap); I resize-pane -t alpha:one.0 -x 8; wait_change "$p"
grid "even-horizontal after resize-pane -x 8"
p=$(snap); I select-layout -E -t alpha:one; wait_change "$p"
grid "select-layout -E  (spread out again)"

I kill-server 2>/dev/null
