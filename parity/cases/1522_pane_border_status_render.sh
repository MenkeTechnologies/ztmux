# pane-border-status and pane-border-format, as an attached client paints them.
#
# The option pair is fully invisible to the server: show-options can echo
# `pane-border-status top` back all day while nothing is drawn. The text only
# exists because screen_redraw_draw_pane_status expands pane-border-format per
# pane through format_draw and blits it INTO the border row -- so the format
# expansion, the border row it is written to, the leading/trailing border fill
# around it, the junction glyph at the column where two status rows meet, and
# the style the text inherits are all client-only behaviour.
#
# Built like 1504/1507/1508: a second server inside a pane of the first, with a
# client attached, so capture-pane on the OUTER server reads back what the inner
# client drew.
#
# Pinned here:
#   * status=top   -- the format lands on each pane's TOP border row, and the
#                     window's own top row becomes a border row (panes lose a line)
#   * status=bottom-- it lands on the BOTTOM row instead, including the window's
#                     last row, which is a different code path in the same fn
#   * status=off   -- restores plain borders with no text
#   * the fill: format_draw centres nothing, it left-aligns after a 2-cell border
#     lead-in and pads the remainder with the border glyph
#   * #[reverse]/#[default] INSIDE pane-border-format, i.e. that the status text
#     goes through format_draw's style parser and not through a plain string copy
#   * the border COLOUR either side of the text: tmux's default
#     pane-active-border-style is fg=themegreen and pane-border-style is
#     fg=themelightgrey, theme colours that resolve only at render time
#     (tty_map_theme_colour), so a port that stores them and never maps them
#     draws an uncoloured border while every show-options case still passes
#
# Determinism: pane titles are set explicitly -- the DEFAULT pane-border-format
# interpolates #{pane_title}, which starts out as the HOSTNAME and would differ
# per machine. No clock, tty or pid is printed. Every wait polls the screen.
set -- $TM
BIN="$1"
ISOCK="pbs_$$_inner"
I() { $BIN -L "$ISOCK" "$@"; }
# Belt and braces: if the harness timeout kills this script mid-way, the inner
# server would otherwise outlive the run.
trap '$BIN -L "$ISOCK" kill-server 2>/dev/null' EXIT INT TERM

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
I set -g status off
I set -g status-interval 0
I set -g @ztmux-ratatui off

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

# Runs of spaces and box-drawing glyphs collapse to <char>{xN} so the border fill
# either side of the status text is measured exactly without an 80-column dump.
grid() {
  echo "== $1"
  $TM capture-pane -p -t client \
    | perl -CSD -pe 's/([ \x{2500}-\x{257f}])\1+/sprintf("%s{x%d}",$1,length($&))/ge; s/[ \t]+$//' \
    | uniq -c | perl -pe 's/^\s*(\d+)\s/[$1] /'
}
# Same collapsing, but keeping SGR: <ESC> stands in for the escape byte.
attrs() {
  echo "== $1"
  $TM capture-pane -p -e -t client \
    | perl -CSD -pe 's/([ \x{2500}-\x{257f}])\1+/sprintf("%s{x%d}",$1,length($&))/ge; s/\e/<ESC>/g; s/[ \t]+$//' \
    | sed -n "$2"
}

p=$(snap)
I split-window -h -t alpha:one 'sleep 300'
wait_change "$p"
p=$(snap); I split-window -v -t alpha:one.1 'sleep 300'; wait_change "$p"

# Titles, not the hostname default.
I select-pane -t alpha:one.0 -T LEFT
I select-pane -t alpha:one.1 -T RIGHT-TOP
I select-pane -t alpha:one.2 -T RIGHT-BOT
I select-pane -t alpha:one.1
I set -g pane-border-format ' #{pane_index}:#{pane_title} '

p=$(snap); I set -g pane-border-status top; wait_change "$p"
grid "pane-border-status top"
attrs "pane-border-status top, styled rows" '1p;13p'

p=$(snap); I set -g pane-border-status bottom; wait_change "$p"
grid "pane-border-status bottom"
attrs "pane-border-status bottom, styled rows" '12p;24p'

# The format is not a plain string: it goes through format_draw, so #[reverse]
# and #[default] inside it must take effect on the border row.
p=$(snap); I set -g pane-border-status top; wait_change "$p"
p=$(snap)
I set -g pane-border-format '#{?pane_active,#[reverse],}[#{pane_index}/#{pane_width}x#{pane_height}]#[default]'
wait_change "$p"
grid "format with #[reverse] on the active pane"
attrs "format with #[reverse], styled rows" '1p;13p'

# Back off: no text, plain borders, and the panes get their line back.
p=$(snap); I set -g pane-border-status off; wait_change "$p"
grid "pane-border-status off"

I kill-server 2>/dev/null
