# pane-border-lines and pane-border-indicators, as an attached client paints them.
#
# Both options choose GLYPHS or COLOURS on the border, and neither exists
# anywhere except on a drawn screen. Their server side is a single enum that
# show-options echoes back; the whole of the behaviour lives in the client's
# redraw -- screen_redraw_border_set picking a character set (single/double/
# heavy/simple/number), and the redraw build marking arrow cells
# (redraw_mark_border_arrows) and the two-pane colour split
# (redraw_mark_two_pane_colours). A port can accept all five line choices, store
# them, print them back, and still draw single lines for every one; no
# server-side case can tell.
#
# The `number` set is the sharpest of the five: instead of box drawing it paints
# the INDEX OF THE PANE each border cell belongs to, so it also asserts the
# ownership decision inside screen_redraw_cell_border. Getting that wrong is
# silent under single lines and glaring here.
#
# pane-border-indicators is checked on a TWO-pane window on purpose. `colour`
# and `both` only differ from `off`/`arrows` when the window is an exact
# left-right or top-bottom pair (redraw_check_two_pane_colours): in that case the
# single dividing rule is painted half in one pane's style and half in the
# other's, which is why the plain glyphs are identical across all four values and
# only the SGR capture separates them. `arrows`/`both` additionally write a
# direction marker at a computed midpoint that appears in no option value and no
# layout string.
#
# Built like 1504/1507/1508: a second server inside a pane of the first, with a
# client attached, so capture-pane on the OUTER server reads back what the inner
# client drew.
#
# Determinism: no clock, tty, pid or hostname is printed, panes run `sleep 300`
# so they stay blank, and every wait polls the painted screen rather than sleeping.
set -- $TM
BIN="$1"
ISOCK="pbl_$$_inner"
I() { $BIN -L "$ISOCK" "$@"; }
# Belt and braces: if the harness timeout kills this script mid-way, the inner
# server would otherwise outlive the run.
trap '$BIN -L "$ISOCK" kill-server 2>/dev/null' EXIT INT TERM

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
I set -g status off
I set -g status-interval 0
I set -g @ztmux-ratatui off

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"

# The settle check compares the screen WITH attributes: pane-border-indicators
# changes only colours, so a plain capture would not see the repaint at all and
# the poll would spin until it gave up.
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

# Runs of the same character collapse to <char>{xN}. For `number` borders the
# repeated character is a digit rather than a box glyph, so the collapsed class
# here deliberately covers digits too.
grid() {
  echo "== $1"
  $TM capture-pane -p -t client \
    | perl -CSD -pe 's/([ 0-9\x{2500}-\x{257f}])\1+/sprintf("%s{x%d}",$1,length($&))/ge; s/[ \t]+$//' \
    | uniq -c | perl -pe 's/^\s*(\d+)\s/[$1] /'
}
# Same collapsing but keeping SGR; <ESC> stands in for the escape byte.
attrs() {
  echo "== $1"
  $TM capture-pane -p -e -t client \
    | perl -CSD -pe 's/([ 0-9\x{2500}-\x{257f}])\1+/sprintf("%s{x%d}",$1,length($&))/ge; s/\e/<ESC>/g; s/[ \t]+$//' \
    | sed -n "$2"
}

# Two panes side by side. The screen is blank until the client attaches and
# paints, so this first wait covers attach and first paint as well as the split.
p=$(snap)
I split-window -h -t alpha:one 'sleep 300'
wait_change "$p"

# `colour` is the default, so the loop starts at `arrows` and comes back round to
# `colour`, which also asserts that turning indicators off and on again restores
# the original picture exactly. Rows 1, 2, 12 and 24 sample above, at and below
# the arrow midpoint and at the far end of the rule, where the two-pane colour
# split has changed style.
for D in arrows both off colour; do
  p=$(snap); I set -g pane-border-indicators "$D"; wait_change "$p"
  attrs "pane-border-indicators $D" '1p;2p;12p;24p'
done

# Now a third pane, stacked inside the right one: that yields a horizontal rule
# and a left-tee on top of the vertical rule, and two distinct pane indices
# meeting at the tee for the `number` set to attribute.
p=$(snap); I split-window -v -t alpha:one.1 'sleep 300'; wait_change "$p"

# Start from `double` so the first assignment is a real repaint; `single` is the
# default and setting it first would change nothing for the wait to detect.
for L in double heavy simple number single; do
  p=$(snap); I set -g pane-border-lines "$L"; wait_change "$p"
  grid "pane-border-lines $L"
done
attrs "pane-border-lines single, styled rows" '1p;13p'

I kill-server 2>/dev/null
