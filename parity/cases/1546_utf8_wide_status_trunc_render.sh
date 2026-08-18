# Double-width characters at the status-left / status-right truncation boundary,
# as the status bar is actually PAINTED.
#
# status-left-length and status-right-length are column budgets, and the string
# is cut by format_trim_left / format_trim_right (format-draw.c). Both count
# COLUMNS, and both DROP a character that would straddle the boundary rather than
# emitting half of it or padding out the gap: format_trim_left copies a character
# only `if (width + ud.width <= limit)` but still adds its width, so the loop then
# exits and an odd budget in front of a wide character yields a string one column
# SHORT of the limit -- and nothing is inserted to make up for it, so the window
# list starts one column earlier than the budget suggests.
#
# That last part is why this has to be read off a rendered bar rather than from
# show-options: the observable difference between "dropped" and "padded" is where
# the window list begins, which only exists on a drawn status line. A port that
# truncates by BYTES or by CHARACTERS instead of columns, or that pads the gap,
# produces a status line that show-options cannot tell apart from the right one.
#
# Three phases, each with a plain-ASCII sequencing tag on the opposite side of
# the bar so the case can poll for the redraw it is waiting for instead of
# sleeping blindly:
#   1. status-left is 10 wide chars plus '|' (21 columns), status-left-length
#      swept across the odd/even boundary
#   2. status-right is the same string, status-right-length swept the same way --
#      a different function, and one that cuts from the OTHER end
#   3. status-left mixes a style and wide characters, so the truncation has to
#      walk over a '#[...]' that costs no columns
#
# Read back the way cases 1504/1507/1508 do it: a second server inside a pane of
# the first with a client attached, so capture-pane on the OUTER server returns
# exactly what the inner client painted.
set -- $TM
BIN="$1"
ISOCK="uws_$$_inner"
# Kill the inner server even if the harness timeout cuts this script short, so a
# slow run cannot leave a server (and its sleeps) behind for the rest of the suite.
trap '$BIN -L "$ISOCK" kill-server >/dev/null 2>&1' EXIT INT TERM

W='\344\270\255'                                # U+4E2D, East Asian Wide
CJK=$(printf "${W}${W}${W}${W}${W}${W}${W}${W}${W}${W}")

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
i=0
while [ $i -lt 150 ]; do
  g=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{window_width}x#{window_height}' 2>/dev/null)
  [ "$g" = "80x23" ] && break
  i=$((i+1)); sleep 0.1
done
echo "geometry: $g"

# Wait for the redraw that carries tag $1 at the END of the status line, then
# print the whole line. Consecutive budgets can render IDENTICALLY (5 and 4 both
# fit two wide chars), so "wait until the line changes" would hang; the tag is
# what makes each redraw individually identifiable.
show_tag_right() {
  local tag="$1" i=0 line=
  while [ $i -lt 200 ]; do
    line=$($TM capture-pane -p -t client | tail -1)
    case "$line" in *"$tag") break ;; esac
    i=$((i+1)); sleep 0.1
  done
  printf '%s\n' "$line" | cat -v
}
show_tag_left() {
  local tag="$1" i=0 line=
  while [ $i -lt 200 ]; do
    line=$($TM capture-pane -p -t client | tail -1)
    case "$line" in "$tag"*) break ;; esac
    i=$((i+1)); sleep 0.1
  done
  printf '%s\n' "$line" | cat -v
}

echo "status-left-length sweep (left = 10 wide chars then '|', 21 columns):"
$BIN -L "$ISOCK" set -g status-left "$CJK|"
$BIN -L "$ISOCK" set -g status-right-length 20
for n in 1 2 3 4 5 6 7 20 21 22; do
  $BIN -L "$ISOCK" set -g status-left-length "$n"
  $BIN -L "$ISOCK" set -g status-right "L$n"
  printf 'left-length=%-2s ' "$n"
  show_tag_right "L$n"
done

echo "status-right-length sweep (right = the same 21 columns):"
$BIN -L "$ISOCK" set -g status-left-length 20
$BIN -L "$ISOCK" set -g status-right "$CJK|"
for n in 1 2 3 4 5 6 7 20 21 22; do
  $BIN -L "$ISOCK" set -g status-right-length "$n"
  $BIN -L "$ISOCK" set -g status-left "R$n "
  printf 'right-length=%-2s ' "$n"
  show_tag_left "R$n "
done

# A style in front of the wide run: '#[fg=red]' costs no columns, so the budget
# has to be spent entirely on the wide characters after it. This walks
# format_trim_left's leading-hash / '#[' branch and its wide-character branch in
# the same string.
echo "styled status-left, budget straddling a wide char:"
$BIN -L "$ISOCK" set -g status-right-length 20
for n in 3 4 5; do
  $BIN -L "$ISOCK" set -g status-left-length "$n"
  $BIN -L "$ISOCK" set -g status-left "#[fg=red]$CJK"
  $BIN -L "$ISOCK" set -g status-right "S$n"
  printf 'styled-length=%-2s ' "$n"
  show_tag_right "S$n"
done
echo "styled status-left with attributes:"
$BIN -L "$ISOCK" set -g status-left-length 5
$BIN -L "$ISOCK" set -g status-right "S9"
i=0
while [ $i -lt 200 ]; do
  line=$($TM capture-pane -p -e -t client | tail -1)
  case "$line" in *"S9"*) break ;; esac
  i=$((i+1)); sleep 0.1
done
printf '%s\n' "$line" | cat -v

$BIN -L "$ISOCK" kill-server 2>/dev/null
