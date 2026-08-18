# Double-width CJK as a CLIENT paints it: where the wrap happens, and what is
# left in the column a wide character cannot fit into.
#
# tmux wraps BEFORE a cell that would not fit, not after: screen_write_cell
# (screen-write.c) tests `s->cx > sx - width`, so in a pane of ODD width the
# final column can never hold a double-width character -- it stays blank and the
# character moves to the next row. A port that keeps the ordinary `cx > sx - 1`
# test writes the wide character one column early, and every column after it on
# that row -- including the pane border -- shifts by one.
#
# None of that is visible from the server side: capture-pane on the inner server
# re-serialises the grid by RUN, so a one-column shift and a padding cell come
# back as the same bytes (both are printed at the bottom of this case to show
# exactly that). The alignment only exists in what the client emitted, so this
# reads it back the way cases 1504/1507/1508 do -- a second server inside a pane
# of the first with a client attached -- and captures the OUTER pane, which is
# the inner client's real output re-parsed into a grid.
#
# The layout covers both parities of pane width at once, with a pane border
# immediately after the odd pane so a mis-sized wide cell would have to displace
# the border glyph:
#
#   pane 0: 59 columns (odd)  -> 29 wide chars = 58 columns, column 59 blank
#   border: column 60
#   pane 1: 20 columns (even) -> 10 wide chars = 20 columns, exact fill
set -- $TM
BIN="$1"
ISOCK="uww_$$_inner"
# Kill the inner server even if the harness timeout cuts this script short, so a
# slow run cannot leave a server (and its sleeps) behind for the rest of the suite.
trap '$BIN -L "$ISOCK" kill-server >/dev/null 2>&1' EXIT INT TERM

W='\344\270\255'                      # U+4E2D, East Asian Wide
rep() { local n="$1" s='' i=0; while [ "$i" -lt "$n" ]; do s="$s$W"; i=$((i+1)); done; printf '%s' "$s"; }
trim() { perl -pe 's/[ \t]+$//'; }
L29=$(rep 29); R10=$(rep 10); R9=$(rep 9)

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"

# Poll for the inner window to take the client's geometry instead of sleeping:
# the split sizes below are only correct once the attach has resized it, and
# under suite load a fixed sleep races that resize.
i=0
while [ $i -lt 150 ]; do
  g=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{window_width}x#{window_height}' 2>/dev/null)
  [ "$g" = "80x23" ] && break
  i=$((i+1)); sleep 0.1
done
echo "geometry: $g"

$BIN -L "$ISOCK" split-window -h -d -l 20 -t alpha:one 'sleep 300'
echo "panes:"
$BIN -L "$ISOCK" list-panes -t alpha:one -F '#{pane_index} #{pane_width}x#{pane_height} left=#{pane_left} right=#{pane_right}'

# Respawn rather than passing the text to split-window, so it is written into a
# pane that ALREADY has its final width -- no reflow, and no dependence on when
# the child started relative to the resize.
#
# Left pane, 59 wide: 30 wide chars. 29 fill columns 1-58, the 30th cannot fit
# in column 59 so it wraps; the trailing Z pins where the second row starts.
$BIN -L "$ISOCK" respawn-pane -k -t alpha:one.0 "printf '${L29}${W}Z'; sleep 300"
# Right pane, 20 wide: an exact fill (10 wide chars), then a row that ends with
# exactly one spare column -- 9 wide chars plus 'A' is 19 columns, so the wide
# char that follows wraps and column 20 stays blank.
$BIN -L "$ISOCK" respawn-pane -k -t alpha:one.1 "printf '${R10}\r\n${R9}A${W}Z'; sleep 300"

# Wait for both panes' output to have reached the outer grid: two Z markers.
i=0
while [ $i -lt 200 ]; do
  n=$($TM capture-pane -p -t client | tr -cd 'Z' | wc -c | tr -d ' ')
  [ "$n" = 2 ] && break
  i=$((i+1)); sleep 0.1
done

# cat -v so a shifted column shows up as a byte difference rather than as a
# terminal-dependent glyph, and od -c on the two load-bearing rows so the blank
# column before the border is asserted as bytes.
echo "rendered rows:"
$TM capture-pane -p -t client | sed -n '1,4p' | cat -v | trim
echo "row1 bytes:"
$TM capture-pane -p -t client | sed -n '1p' | od -c
echo "row2 bytes:"
$TM capture-pane -p -t client | sed -n '2p' | od -c

# The same rows straight from the inner server: the blank column and the border
# are absent there, which is why the rendered capture above is the assertion.
echo "inner grid, left pane:"
$BIN -L "$ISOCK" capture-pane -p -t alpha:one.0 | sed -n '1,2p' | cat -v
echo "inner grid, right pane:"
$BIN -L "$ISOCK" capture-pane -p -t alpha:one.1 | sed -n '1,3p' | cat -v
echo "inner cursors:"
$BIN -L "$ISOCK" display-message -p -t alpha:one.0 'left=#{cursor_x},#{cursor_y}'
$BIN -L "$ISOCK" display-message -p -t alpha:one.1 'right=#{cursor_x},#{cursor_y}'

$BIN -L "$ISOCK" kill-server 2>/dev/null
