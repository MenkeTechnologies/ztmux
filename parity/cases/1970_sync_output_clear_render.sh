# Screen clears inside a synchronized-output block (DEC mode 2026), as an
# attached client paints them.
#
# The three clear-screen operations draw through a gate like every other
# multi-row operation: during a sync block they write nothing to the tty and
# record the rows they cleared instead (screen-write.c:1934 clearendofscreen,
# :2009 clearstartofscreen, :2082 clearscreen), and the block end redraws those
# rows. A port that writes the clear straight to the tty mid-block paints a
# frame the application never meant to show; one that records too few rows
# leaves cleared text on the client.
#
# Each clear runs in its own sync block and is followed, in the same block, by a
# marker written on a row the clear did NOT touch plus one it did, so both a
# missing redraw and a lost clear show in the painted grid:
#
#   ED 0 (below)  with the cursor mid-screen
#   ED 1 (above)  with the cursor mid-screen
#   ED 2 (all)    with the cursor at home, then the screen refilled
#
# Each block is split by a keypress after its clear, so the painted screen is
# also captured MID-block: it must still show the frame from before the clear.
# A port that writes the clear straight to the tty shows it there early, which
# the final picture alone cannot reveal. The program releases each half-block
# on a keypress, and every wait polls either the inner cursor (mid-block) or the
# painted screen (block end), so nothing waits on a blind sleep.
set -- $TM
BIN="$1"
ISOCK="syc_$$_inner"
WORK=$(mktemp -d)
trap '$BIN -L "$ISOCK" kill-server >/dev/null 2>&1; rm -rf "$WORK"' EXIT INT TERM

cat > "$WORK/clear.pl" <<'EOF'
$| = 1;
system "stty raw -echo";
sub row { printf "\e[%d;1H\e[K%s", $_[0], $_[1] }
sub step { sysread STDIN, my $c, 1 }
sub fill { row($_, sprintf "%s %02d", $_[0], $_) for 1 .. 20 }
print "\e[H\e[2J";
fill("a");
row(22, "READY");

step();
print "\e[?2026h\e[10;5H\e[0J";
step();
row(3, "ED0 kept-above");
print "\e[?2026l";

step();
print "\e[?2026h";
fill("b");
print "\e[12;5H\e[1J";
step();
row(15, "ED1 kept-below");
print "\e[?2026l";

step();
print "\e[?2026h\e[H\e[2J";
step();
row($_, sprintf "c %02d", $_) for 4 .. 8;
row(22, "DONE");
print "\e[?2026l";
EOF

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status off
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"

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

# The clear has been applied once the inner cursor sits where its block moved
# it; the painted screen must not have changed yet. Sync mode holds for up to a
# second (screen-write.c:1032), far longer than this poll and one capture.
wait_cursor() {
  i=0
  while [ $i -lt 300 ]; do
    [ "$($BIN -L "$ISOCK" display-message -p -t alpha:one '#{cursor_y},#{cursor_x}')" = "$1" ] && return 0
    i=$((i+1)); sleep 0.02
  done
  echo "wait_cursor $1: TIMED OUT"
}

$BIN -L "$ISOCK" respawn-pane -k -t alpha:one "perl $WORK/clear.pl; sleep 300"
wait_for '^READY'
for step in "9,4 ^ED0 kept-above" "11,4 ^ED1 kept-below" "0,0 ^DONE"; do
  cur=${step%% *} want=${step#* }
  $BIN -L "$ISOCK" send-keys -t alpha:one x
  wait_cursor "$cur"
  echo "== painted mid-block, before $want (the block has not ended)"
  $TM capture-pane -p -t client | perl -pe 's/[ \t]+$//' | head -22
  $BIN -L "$ISOCK" send-keys -t alpha:one x
  wait_for "$want"
  echo "== painted after $want"
  $TM capture-pane -p -t client | perl -pe 's/[ \t]+$//' | head -22
done

echo "== inner grid"
$BIN -L "$ISOCK" capture-pane -p -t alpha:one | perl -pe 's/[ \t]+$//' | head -22

$BIN -L "$ISOCK" kill-server 2>/dev/null
