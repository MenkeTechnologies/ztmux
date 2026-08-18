# status-left-length / status-right-length, and what happens to the window list
# when the two ends leave it no room.
#
# Both options are pure render-time arithmetic. The server stores two numbers and
# the full untruncated status-left/status-right strings, so show-options parity
# says nothing at all about them: a port that ignores the limits entirely, or
# truncates from the wrong end, or treats 0 as "show nothing" instead of "no
# limit", still round-trips every option perfectly.
#
# The default status-format[0] applies them as `#{T;=/#{status-left-length}:...}`,
# so this also pins that the format's `=/N` truncation is wired to the option, and
# -- once the ends are wide enough -- the list-left-marker `<` / list-right-marker
# `>` that status_redraw substitutes when the window list has to be clipped around
# the current window.
#
# Built like cases 1504/1507/1508: a second server inside a pane of the first with
# a client attached, so capture-pane on the OUTER server reads back the inner
# client's status row.
set -- $TM
BIN="$1"
ISOCK="slt_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
for n in two three four five six seven eight; do
  $BIN -L "$ISOCK" new-window -d -t alpha -n "$n" 'sleep 300'
done
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the PORTED
# rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" set -g default-terminal screen-256color
$TM set -g default-terminal screen-256color
# Long enough that every limit below actually cuts, and distinct per side so a
# swapped left/right is visible.
$BIN -L "$ISOCK" set -g status-left  'LEFT0123456789abcdefghijklmnopqrstuvwxyz'
$BIN -L "$ISOCK" set -g status-right 'RIGHT0123456789abcdefghijklmnopqrstuvwxyz'

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"

# Fence instead of sleeping: under suite load a blind sleep races the repaint.
# The lengths are set FIRST and a marker is stamped into the CURRENT window's
# entry last -- the current entry is the one status_redraw always keeps on
# screen, so the marker survives even the steps where the list is clipped.
waitfor() {  # $1 = text that must appear on the inner client's screen
  local i=0 got
  while [ $i -lt 80 ]; do
    got=$($TM capture-pane -p -t client 2>/dev/null)
    case "$got" in *"$1"*) sleep 0.3; return 0 ;; esac
    i=$((i+1)); sleep 0.1
  done
  echo "waitfor: timed out waiting for [$1]"
  return 1
}
fence() { $BIN -L "$ISOCK" set -g window-status-current-format "#{window_index}:#{window_name}*$1"; waitfor "$1"; }
bar() { $TM capture-pane -p -t client | tail -1 | cat -v | perl -pe 's/[ \t]+$//'; }

echo "defaults:  $($BIN -L "$ISOCK" show -g status-left-length) / $($BIN -L "$ISOCK" show -g status-right-length)"

# 1. Stock 10 / 40. Left keeps its first 10 columns, right its first 40 and is
#    flush to column 80; the list in between is already too narrow and ends in >.
fence '=A'
echo "10/40 : [$(bar)]"

# 2. Tight limits on both ends -- the list now has room to spare.
$BIN -L "$ISOCK" set -g status-left-length 5
$BIN -L "$ISOCK" set -g status-right-length 4
fence '=B'
echo "5/4   : [$(bar)]"

# 3. One column each: the smallest non-zero limit, not a special case.
$BIN -L "$ISOCK" set -g status-left-length 1
$BIN -L "$ISOCK" set -g status-right-length 1
fence '=C'
echo "1/1   : [$(bar)]"

# 4. Zero means NO limit, not "empty" -- both ends print in full and collide in
#    the middle, squeezing the window list out entirely.
$BIN -L "$ISOCK" set -g status-left-length 0
$BIN -L "$ISOCK" set -g status-right-length 0
# No list left to carry a marker here, so fence on the collision itself: the two
# full-length ends meeting mid-line is a string no other step can produce.
waitfor 'wxyzIGHT'
echo "0/0   : [$(bar)]"

# 5. Wide-but-limited ends: the list is clipped and the right marker appears.
$BIN -L "$ISOCK" set -g status-left-length 30
$BIN -L "$ISOCK" set -g status-right-length 30
fence '=E'
echo "30/30 first current : [$(bar)]"

# 6. Same, with the LAST window current: the clip window slides and the left
#    marker appears instead.
$BIN -L "$ISOCK" select-window -t alpha:7
fence '=F'
echo "30/30 last current  : [$(bar)]"

$BIN -L "$ISOCK" kill-server 2>/dev/null
