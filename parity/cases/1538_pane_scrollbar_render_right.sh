# The pane scrollbar as a CLIENT actually paints it, on the right.
#
# The scrollbar is never in the pane's grid: redraw_draw_scrollbar_span
# (screen-redraw.c:1230) writes it straight to the tty of an attached client, so
# capture-pane on the pane itself can never see it and nothing in the suite had
# looked at it. What that function computes is the slider: outside a mode it is
# sized by screen height over screen+history and pinned to the BOTTOM
# (slider_y = sb_h - slider_h, screen-redraw.c:1252), while inside copy mode it
# is positioned from the copy offset by a DIFFERENT formula,
# slider_y = (sb_h + 1) * cm_y / total_height (screen-redraw.c:1263) -- note the
# sb_h + 1, which is why the two modes do not agree at the same scroll position.
# Slider cells are the style with fg and bg SWAPPED (slgc, screen-redraw.c:1274);
# trough cells are the style as written.
#
# Read through a nested server, the way cases 1504/1507/1508 do: an inner server
# runs inside a pane of the outer one with a client attached, so capture-pane on
# the OUTER pane reads back the bytes the inner client painted, scrollbar
# included.
set -- $TM
BIN="$1"
ISOCK="sbr_$$_inner"

# 60 lines of output into a 23-row pane leaves exactly 38 lines of history, so
# the slider covers 23/(23+38) of the bar. Named colours (not the themed
# defaults) keep the capture to short SGR codes that cannot collide with the
# status line's.
$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sh -c "seq 60; sleep 300"'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" set -gw pane-scrollbars on
$BIN -L "$ISOCK" set -gw pane-scrollbars-position right
$BIN -L "$ISOCK" set -gw pane-scrollbars-style 'bg=red,fg=green,width=1,pad=0'

# Reduce the 23 pane rows to one character each: "-" trough, "S" slider,
# "." neither. Nothing but the scrollbar is red or green, so a plain substring
# test is enough and the result is independent of where in the row it landed.
bar() {
  $TM capture-pane -p -e -t client | sed -n '1,23p' | perl -ne '
    chomp; my $c = ".";
    $c = "-" if index($_, "\e[32m\e[41m") >= 0;
    $c = "S" if index($_, "\e[31m\e[42m") >= 0;
    print $c; END { print "\n" }'
}

# Never sleep blind waiting for a draw. Wait for the server-side fact first,
# then poll the painted bar until it has both left its previous value and
# repeated itself once -- so a half-finished repaint cannot be sampled.
wait_state() {  # $1 = format, $2 = expected value
  local i=0 got
  while [ $i -lt 200 ]; do
    got=$($BIN -L "$ISOCK" display -p -t alpha:one "$1" 2>/dev/null)
    [ "$got" = "$2" ] && return 0
    i=$((i+1)); sleep 0.05
  done
  echo "wait_state: [$1] wanted [$2] got [$got]"
  return 1
}
settle() {      # $1 = the bar this must no longer be
  local not="$1" prev="" cur="" i=0
  while [ $i -lt 200 ]; do
    cur=$(bar)
    if [ "$cur" != "$not" ] && [ "$cur" = "$prev" ]; then printf '%s\n' "$cur"; return 0; fi
    prev="$cur"; i=$((i+1)); sleep 0.05
  done
  printf '%s <never settled>\n' "$cur"
}

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
wait_state '#{pane_width}x#{pane_height}/#{history_size}' '79x23/38'

# Outside any mode: slider sized 23*23/61 = 8 rows, sitting at the bottom.
b0=$(settle '.......................')
echo "no mode:        $b0"

# Copy mode at the very bottom. Same scroll position as above, but the copy-mode
# formula puts the slider one row higher -- that off-by-one is the whole reason
# both branches are worth pinning.
$BIN -L "$ISOCK" copy-mode -t alpha:one
wait_state '#{pane_mode}' copy-mode
wait_state '#{scroll_position}' 0
b1=$(settle "$b0")
echo "copy bottom:    $b1"

# Scrolled to the very top of the history.
$BIN -L "$ISOCK" send-keys -t alpha:one -X history-top
wait_state '#{scroll_position}' 38
b2=$(settle "$b1")
echo "copy top:       $b2"

# ... and part way, which is the only place the multiply-then-truncate in
# slider_y can differ from a rounded or a floating result.
$BIN -L "$ISOCK" send-keys -t alpha:one -X -N 19 scroll-down
wait_state '#{scroll_position}' 19
b3=$(settle "$b2")
echo "copy middle:    $b3"

$BIN -L "$ISOCK" send-keys -t alpha:one -X cancel
wait_state '#{pane_mode}' ''
b4=$(settle "$b3")
echo "back to no mode:$b4"

# With the option off the pane is full width again and NOTHING is painted in the
# scrollbar colours -- the trough must not linger.
$BIN -L "$ISOCK" set -gw pane-scrollbars off
wait_state '#{pane_width}' 80
echo "scrollbars off: $(settle "$b4")"

# "modal" and "auto-hide" are overlay states: the pane stays 80 wide because no
# column is reserved for them.
$BIN -L "$ISOCK" set -gw pane-scrollbars modal
wait_state '#{pane_width}' 80 && echo "modal width:    80"
$BIN -L "$ISOCK" set -gw pane-scrollbars auto-hide
wait_state '#{pane_width}' 80 && echo "autohide width: 80"

$BIN -L "$ISOCK" kill-server 2>/dev/null
