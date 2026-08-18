# display-popup AS THE CLIENT DRAWS IT: its box, its geometry, the pty size the
# command inside actually gets, and -E's close-on-exit behaviour.
#
# Case 1466 pins display-popup's ARGUMENT PARSER only -- with no client every
# form stops at "no current client", so -w/-h/-x/-y have never been observed
# actually placing anything, and popup_draw_cb has never run in the suite at all.
# What this pins:
#   * default size is exactly half the terminal in each axis and centred
#     (cmd-display-menu.c:414/423, w = sx/2, h = sy/2, popup_centre_x/y).
#   * -w/-h in absolute cells and in per-cent (args_percentage against sx/sy).
#   * -y names the BOTTOM edge, so the box top is y-h (cmd-display-menu.c:262),
#     and both axes clamp when the box would run off the terminal.
#   * the command inside runs on a pty sized to the popup's INTERIOR, which
#     `stty size` reports -- an off-by-one in the border inset shows up here and
#     nowhere else.
#   * -E closes the popup when the command exits; without -E the popup stays up
#     showing the finished command's output.
#
# Built like cases 1504/1507/1508: an inner server whose attached client lives in
# a pane of the outer server, so capture-pane on the OUTER server reads back what
# the inner client painted.
set -- $TM
BIN="$1"
ISOCK="dpr_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
$BIN -L "$ISOCK" set -g @ztmux-ratatui off

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"

# Poll the drawn screen instead of sleeping: this case runs alongside ~1200
# others, so a blind sleep races the client's paint under load. Once the
# expected text is there, wait until two consecutive captures agree, so a
# half-painted overlay can never be read back either.
snap()      { $TM capture-pane -p -t client; }
settle()    { local a b i=0; a=$(snap); while [ $i -lt 100 ]; do sleep 0.05; b=$(snap);
                [ "$a" = "$b" ] && return 0; a=$b; i=$((i+1)); done; }
wait_for()  { local i=0; while [ $i -lt 300 ]; do
                snap | grep -qF "$1" && { settle; return 0; }
                i=$((i+1)); sleep 0.05; done; echo "wait_for TIMEOUT [$1]"; return 1; }
wait_gone() { local i=0; while [ $i -lt 300 ]; do
                snap | grep -qF "$1" || { settle; return 0; }
                i=$((i+1)); sleep 0.05; done; echo "wait_gone TIMEOUT [$1]"; return 1; }

# Where the box is, in rows/columns, without depending on how many bytes a box
# glyph takes: U+250C is E2 94 8C and U+2514 is E2 94 94 in UTF-8.
geom() {
  $TM capture-pane -p -t client | perl -ne '
    chomp;
    if (/^( *)\xe2\x94\x8c/) { $top = $.; $col = length($1); }
    if (/^ *\xe2\x94\x94/)   { $bot = $.; }
    END { printf "top-row=%s bottom-row=%s left-col=%s\n",
                 $top // "none", $bot // "none", $col // "none"; }'
}

# The popup command runs `stty size`, which prints "<rows> <cols>" for the pty
# it owns. Strip the box glyphs (non-ASCII) so only the text inside survives.
pty_size() {
  $TM capture-pane -p -t client | perl -ne '
    s/[^\x20-\x7e]//g; s/^\s+//; s/\s+$//;
    print "pty-rows-cols=[$_]\n" if /^\d+ \d+$/'
}

# `stty size` prints "<rows> <cols>" for the pty the popup command owns.
open_popup() { $BIN -L "$ISOCK" bind p display-popup "$@" -T Pop \
                 'stty size; printf "READY\r\n"; sleep 300'
               $TM send-keys -t client C-b p
               wait_for READY; }
close_popup() { $BIN -L "$ISOCK" display-popup -C -t alpha:one; wait_gone READY; }

wait_for '[alpha] 0:one*'

echo "== default size (half the 80x24 terminal, centred) =="
open_popup -E
$TM capture-pane -p -t client | cat -v | perl -ne 'printf "%2d|%s", $., $_'
close_popup

echo "== -w 20 -h 6 =="
open_popup -E -w 20 -h 6
geom; pty_size
close_popup

echo "== -w 50% -h 25% =="
open_popup -E -w 50% -h 25%
geom; pty_size
close_popup

echo "== -w 100% -h 100% (clamped to the terminal) =="
open_popup -E -w 100% -h 100%
geom; pty_size
close_popup

echo "== -w 30 -h 8 -x 40 -y 20 (-y is the BOTTOM edge) =="
open_popup -E -w 30 -h 8 -x 40 -y 20
geom; pty_size
close_popup

echo "== -w 30 -h 8 -x 60 -y 4 (clamped: x+w and y-h both run off) =="
open_popup -E -w 30 -h 8 -x 60 -y 4
geom; pty_size
close_popup

# -E closes the popup as soon as the command exits.
echo "== -E, command exits, popup goes away =="
$BIN -L "$ISOCK" bind q display-popup -E -w 20 -h 5 -T Bye 'printf "BYEBYE\r\n"'
$TM send-keys -t client C-b q
wait_gone 'BYEBYE'
geom

# Without -E the popup stays up with the finished command's output on screen.
echo "== no -E, command exits, popup stays =="
$BIN -L "$ISOCK" bind r display-popup -w 20 -h 5 -T Stay 'printf "STAYPUT\r\n"'
$TM send-keys -t client C-b r
wait_for 'STAYPUT'
geom
$TM capture-pane -p -t client | perl -ne 's/[^\x20-\x7e]//g; s/^\s+//; s/\s+$//;
                                          print "inner=[$_]\n" if /^STAYPUT$/'
echo "== display-popup -C closes it =="
$BIN -L "$ISOCK" display-popup -C -t alpha:one
wait_gone 'STAYPUT'
geom

$BIN -L "$ISOCK" kill-server 2>/dev/null
