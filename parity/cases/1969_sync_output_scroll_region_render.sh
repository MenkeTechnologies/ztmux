# Region scrolls inside a synchronized-output block (DEC mode 2026), as an
# attached client paints them.
#
# Inside a sync block nothing reaches the tty: screen_write_should_draw_lines
# (screen-write.c:220) records the rows each operation touched in the pane's
# sync_dirty bitmap, and screen_write_stop_sync redraws exactly those rows when
# the block ends. Every multi-row operation therefore has to mark its whole
# affected range -- deleteline/reverseindex/scrolldown the scroll region,
# insertline the cursor row down to the region bottom. Marking only the cursor
# row leaves every row the operation shifted stale on the client while the
# grid (and so capture-pane on the inner server) is correct.
#
# This is how nvim scrolls: `ESC[?2026h`, set the region above the statusline,
# DL/IL or SU inside it, repaint the rows that scrolled in, `ESC[?2026l`. With
# the dirty range wrong, a mouse-wheel scroll in nvim moves only the top row and
# the rows nvim repainted; the middle of the window stays frozen.
#
# The pane program below drives each scroll primitive (DL, SU, LF at the region
# bottom, IL, RI) inside its own sync block, and the painted grid is read back
# from the OUTER server the way 1504/1544 do. The program releases one block per
# keypress and every wait polls the painted screen for the row that block wrote,
# so each block is its own read (one burst would be absorbed by the initial
# full-pane redraw) and no wait is a blind sleep.
set -- $TM
BIN="$1"
ISOCK="syr_$$_inner"
WORK=$(mktemp -d)
trap '$BIN -L "$ISOCK" kill-server >/dev/null 2>&1; rm -rf "$WORK"' EXIT INT TERM

cat > "$WORK/scroll.pl" <<'EOF'
$| = 1;
system "stty raw -echo";
my $R = 20;    # region rows 1..20; row 21 is a fixed "statusline"
my $n = 0;
sub row { printf "\e[%d;1H\e[Kline %04d", $_[0], $_[1] }
sub step { sysread STDIN, my $c, 1 }
print "\e[H\e[2J";
row($_, ++$n) for 1 .. $R;
print "\e[21;1HSTATUS";
my %scroll = (
    dl => "\e[1;${R}r\e[1;1H\e[3M",
    su => "\e[1;${R}r\e[3S",
    lf => "\e[1;${R}r\e[${R};1H\n\n\n",
);
for my $op (qw(dl su lf)) {
    step();
    print "\e[?2026h$scroll{$op}\e[r";
    row($R - 3 + $_, ++$n) for 1 .. 3;
    print "\e[?2026l";
}
step();
print "\e[?2026h\e[1;${R}r\e[1;1H\e[2L\e[r";
row(1, 9001); row(2, 9002);
print "\e[?2026l";
step();
print "\e[?2026h\e[1;${R}r\e[1;1H\eM\e[r";
row(1, 9100);
print "\e[?2026l";
print "\e[22;1HDONE";
EOF

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status off
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"

# The window must have the client's geometry before the program draws, or the
# attach-time resize repaints the whole pane and hides the bug.
i=0
while [ $i -lt 150 ]; do
  g=$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{window_width}x#{window_height}' 2>/dev/null)
  [ "$g" = "80x24" ] && break
  i=$((i+1)); sleep 0.1
done
echo "geometry: $g"

wait_for() {
  i=0
  while [ $i -lt 150 ]; do
    $TM capture-pane -p -t client | grep -q "$1" && return 0
    i=$((i+1)); sleep 0.1
  done
  echo "wait_for $1: TIMED OUT"
}

$BIN -L "$ISOCK" respawn-pane -k -t alpha:one "perl $WORK/scroll.pl; sleep 300"
wait_for '^STATUS'
for want in 'line 0023' 'line 0026' 'line 0029' 'line 9002' 'line 9100'; do
  $BIN -L "$ISOCK" send-keys -t alpha:one x
  wait_for "^$want"
done
wait_for '^DONE'

echo "== painted by the client"
$TM capture-pane -p -t client | perl -pe 's/[ \t]+$//' | head -22
echo "== inner grid"
$BIN -L "$ISOCK" capture-pane -p -t alpha:one | perl -pe 's/[ \t]+$//' | head -22

$BIN -L "$ISOCK" kill-server 2>/dev/null
