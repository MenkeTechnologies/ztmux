# Combining marks, ZWJ emoji, Hangul jamo and variation selectors: how many
# COLUMNS each of them takes once a client has drawn it.
#
# Case 1470 pins what the server STORES for these sequences -- which bytes land
# in which grid cell. That cannot distinguish a cell of width 1 from a cell of
# width 2: capture-pane re-serialises by run and a padding cell prints as
# nothing, so both come back as identical bytes (the inner-server dump below
# shows exactly that). The width is what screen_write_combine (screen-write.c)
# decides, and it is only observable as the column the NEXT character lands in.
#
# So every test line is padded to end at the pane's right edge. The pane is 20
# columns; each line writes 18 or 19 filler columns, then the sequence under
# test, then 'Q'. Two columns wide -> the sequence fills the pane and Q wraps to
# the following row. One column wide -> Q stays on the row. Which row the Q
# lands on IS the assertion.
#
#   a  a wide char exactly filling the last two columns
#   b  a wide char with one column left: it wraps, the last column stays blank
#   c  a ZWJ sequence (U+1F468 U+200D U+1F4BB) is ONE cell of two columns, not
#      two emoji of two columns each -- #{w:} reports 4 for that same string, so
#      the format width and the drawn width deliberately disagree
#   d  Hangul jamo U+1100 U+1161 compose (hanguljamo_check_state) into a single
#      two-column cell instead of occupying one cell each
#   e  a combining mark (U+0301) adds no column, so base+mark still fits in a
#      single remaining column
#   f  a non-BMP emoji (U+1F680) is two columns
#
# A second window then repeats the same padding trick for a variation selector
# (U+2764 U+FE0F) with variation-selector-always-wide OFF, next to the ZWJ
# sequence as a control: the VS sequence must occupy one column while the ZWJ
# sequence next to it must still occupy two.
#
# Only the OFF state is asserted here. The ON state -- U+2764 U+FE0F widened to
# two columns -- was a divergence when this case was written, so pinning it would
# have made the case fail rather than describe the contract. Once the port agrees,
# the ON half is one more line in the first block, in the same shape as the rest:
#     ${X18}${VS}Q      with the option left at its default
# and Q must land on the following row.
#
# The rendering is read back the way cases 1504/1507/1508 do it: a second server
# inside a pane of the first with a client attached, so capture-pane on the OUTER
# server re-parses exactly the bytes and cursor motion the inner client emitted.
# The pane under test is the LEFT one, so its right edge is mid-screen and not
# the outer terminal's own right edge.
set -- $TM
BIN="$1"
ISOCK="ucz_$$_inner"
# Kill the inner server even if the harness timeout cuts this script short, so a
# slow run cannot leave a server (and its sleeps) behind for the rest of the suite.
trap '$BIN -L "$ISOCK" kill-server >/dev/null 2>&1' EXIT INT TERM

ACU='\314\201'                                      # U+0301 combining acute
W='\344\270\255'                                    # U+4E2D wide
ROCK='\360\237\232\200'                             # U+1F680
ZWJ='\360\237\221\250\342\200\215\360\237\222\273'  # U+1F468 U+200D U+1F4BB
VS='\342\235\244\357\270\217'                       # U+2764 U+FE0F
JAMO='\341\204\200\341\205\241'                     # U+1100 U+1161
xs() { local n="$1" s='' i=0; while [ "$i" -lt "$n" ]; do s="${s}x"; i=$((i+1)); done; printf '%s' "$s"; }
X18=$(xs 18); X19=$(xs 19)
# Keep the left pane and the border glyph, drop the (empty) right pane: the
# border column is then part of the compared output, so a one-column shift in
# the left pane shows up even on rows whose text is short.
left() { perl -pe 's/(\342\224\202).*$/$1/'; }

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"

# Poll for the attach-driven resize rather than sleeping: the split size below is
# only right once the window has taken the client's geometry, and under suite
# load a fixed sleep races that resize.
i=0
while [ $i -lt 150 ]; do
  g=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{window_width}x#{window_height}' 2>/dev/null)
  [ "$g" = "80x23" ] && break
  i=$((i+1)); sleep 0.1
done
echo "geometry: $g"
$BIN -L "$ISOCK" split-window -h -d -l 59 -t alpha:one 'sleep 300'
echo "panes:"
$BIN -L "$ISOCK" list-panes -t alpha:one -F '#{pane_index} #{pane_width}x#{pane_height} left=#{pane_left} right=#{pane_right}'

# Respawn so the text is written into a pane that already has its final width.
$BIN -L "$ISOCK" respawn-pane -k -t alpha:one.0 "printf 'a\r\n${X18}${W}Q\r\nb\r\n${X19}${W}Q\r\nc\r\n${X18}${ZWJ}Q\r\nd\r\n${X18}${JAMO}Q\r\ne\r\n${X19}e${ACU}Q\r\nf\r\n${X18}${ROCK}Q\r\nEND'; sleep 300"
i=0
while [ $i -lt 200 ]; do
  $TM capture-pane -p -t client | grep -q 'END' && break
  i=$((i+1)); sleep 0.1
done
echo "rendered:"
$TM capture-pane -p -t client | sed -n '1,20p' | left | cat -v

# The same rows out of the inner server's own grid: same bytes, no column
# information at all, which is why the capture above is the assertion.
echo "inner grid:"
$BIN -L "$ISOCK" capture-pane -p -t alpha:one.0 | sed -n '1,20p' | cat -v

# Variation selector with the widening option off. It has to go off on the OUTER
# server too, or the outer's own parser would re-widen the sequence while
# re-parsing the inner client's output. A fresh window (rather than reusing the
# pane above) also keeps this to one respawn per pane.
$BIN -L "$ISOCK" set -g variation-selector-always-wide off
$TM set -g variation-selector-always-wide off
$BIN -L "$ISOCK" new-window -d -n two -t alpha 'sleep 300'
$BIN -L "$ISOCK" split-window -h -d -l 59 -t alpha:two 'sleep 300'
$BIN -L "$ISOCK" respawn-pane -k -t alpha:two.0 "printf 'g\r\n${X18}${VS}Q\r\nh\r\n${X18}${ZWJ}Q\r\nEND2'; sleep 300"
$BIN -L "$ISOCK" select-window -t alpha:two
i=0
while [ $i -lt 200 ]; do
  $TM capture-pane -p -t client | grep -q 'END2' && break
  i=$((i+1)); sleep 0.1
done
echo "variation-selector-always-wide off (g = variation selector, h = ZWJ control):"
$TM capture-pane -p -t client | sed -n '1,6p' | left | cat -v

$BIN -L "$ISOCK" kill-server 2>/dev/null
