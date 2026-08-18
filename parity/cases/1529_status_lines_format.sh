# Multi-line status bars: `status 2` / `status 3` and the status-format[N] array.
#
# The number of status lines is not a cosmetic setting -- it comes out of the
# window's usable height, and each line is a separate format expanded from a
# separate array entry. Nothing about that is observable from the server: the
# option array round-trips through show-options whether or not status_redraw ever
# draws line 1 and line 2, and whether or not the pane is actually made shorter
# to make room for them.
#
# So this case pins, from a real attached client: how many rows the bar occupies,
# WHICH row each status-format[N] lands on for status-position top and bottom,
# that #{window_height} shrinks by exactly the number of status lines, and the
# stock status-format[1] / status-format[2] -- the pane list and the session list,
# both of which exist only in this multi-line mode and are rendered by no other
# case here.
#
# Built like cases 1504/1507/1508: a second server inside a pane of the first with
# a client attached, so capture-pane on the OUTER server reads back the inner
# client's rows.
set -- $TM
BIN="$1"
ISOCK="slf_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" split-window -d -t alpha:one 'sleep 300'
$BIN -L "$ISOCK" new-session -d -s beta -n solo -x 80 -y 24 'sleep 300'
# The stock window-pane-status-format prints #T, which defaults to the HOSTNAME.
# Give both panes a fixed title so the stock format can be exercised as-is.
$BIN -L "$ISOCK" select-pane -t alpha:one.0 -T titleA
$BIN -L "$ISOCK" select-pane -t alpha:one.1 -T titleB
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the PORTED
# rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" set -g default-terminal screen-256color
$TM set -g default-terminal screen-256color

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"

# Fence instead of sleeping: under suite load a blind sleep races the repaint.
# Every step configures the lines it is about to read FIRST and stamps a unique
# marker LAST, so the marker can only show up in a redraw that already used the
# new configuration.
fence() {   # $1 = marker string that must appear somewhere on the screen
  local i=0 got
  while [ $i -lt 80 ]; do
    got=$($TM capture-pane -p -t client 2>/dev/null)
    case "$got" in *"$1"*) sleep 0.3; return 0 ;; esac
    i=$((i+1)); sleep 0.1
  done
  echo "fence: timed out waiting for [$1]"
  return 1
}
# Trailing blanks only: \s would eat the newline and run the rows together.
show() { cat -v | perl -pe 's/[ \t]+$//; s/^/  |/'; }

# 1. Two lines at the bottom. status-format[1] is the SECOND line; the first
#    screen row must still be application content.
$BIN -L "$ISOCK" set -g status-format[1] 'line1 h=#{window_height}'
$BIN -L "$ISOCK" set -g status 2
$BIN -L "$ISOCK" set -g status-format[0] 'line0 A1 w=#{window_width}'
fence 'line0 A1'
echo "status 2 bottom, first row + last two:"
$TM capture-pane -p -t client | head -1 | show
$TM capture-pane -p -t client | tail -2 | show

# 2. Three lines. The height must drop by one more than in step 1.
$BIN -L "$ISOCK" set -g status-format[2] 'line2 tail'
$BIN -L "$ISOCK" set -g status 3
$BIN -L "$ISOCK" set -g status-format[0] 'line0 A2 h=#{window_height}'
fence 'line0 A2'
echo "status 3 bottom, last three:"
$TM capture-pane -p -t client | tail -3 | show

# 3. The same three lines at the top: order of the array on screen is the thing
#    being pinned, not just that three rows got used.
$BIN -L "$ISOCK" set -g status-position top
$BIN -L "$ISOCK" set -g status-format[0] 'line0 A3 h=#{window_height}'
fence 'line0 A3'
echo "status 3 top, first three + last:"
$TM capture-pane -p -t client | head -3 | show
$TM capture-pane -p -t client | tail -1 | cat -v | perl -pe 's/[ \t]+$//; s/^/  |last:/'

# 4. The STOCK status-format[1] and [2]: the pane list and the session list, with
#    their alignment prefix, their list-focus styling and their own separators.
#    Captured with -e, so the attributes on the focused entries are compared too.
$BIN -L "$ISOCK" set -g status-position bottom
# `set -gu status-format[N]` blanks that ONE entry; unsetting the whole array is
# what restores the stock three formats.
$BIN -L "$ISOCK" set -gu status-format
$BIN -L "$ISOCK" set -g status-left 'A4| '
fence 'A4|'
echo "status 3 default formats, last three (with attributes):"
$TM capture-pane -p -e -t client | tail -3 | show

$BIN -L "$ISOCK" kill-server 2>/dev/null
