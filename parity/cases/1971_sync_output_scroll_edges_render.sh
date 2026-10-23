# The scroll operations 1969 does not reach, inside synchronized-output blocks
# (DEC mode 2026), as an attached client paints them.
#
# 1969 scrolls a region that starts at the top row. The dirty range each
# operation records is computed from the region bounds and the cursor
# (screen-write.c:1494/1521 insertline, :1745 reverseindex, :1881 scrolldown),
# so a region in the MIDDLE of the screen and a cursor OUTSIDE the region are
# where an off-by-rupper or a cursor-row-only range shows up:
#
#   SD 2   (CSI 2 T)  region rows 5-15: rows 5-6 scroll in blank, 16+ untouched
#   RI     (ESC M)    cursor on the top of a rows 5-15 region
#   IL 2   (CSI 2 L)  inside a rows 5-15 region, cursor on row 9
#   IL 2   (CSI 2 L)  cursor on row 18, BELOW a rows 5-15 region: the
#                     out-of-region path shifts rows 18..bottom
#
# Rows outside the region carry fixed text that must survive every step, and
# each block ends by writing a marker on row 22 that is the readiness signal.
# One block per keypress, so each block is its own read.
set -- $TM
BIN="$1"
ISOCK="sye_$$_inner"
WORK=$(mktemp -d)
trap '$BIN -L "$ISOCK" kill-server >/dev/null 2>&1; rm -rf "$WORK"' EXIT INT TERM

cat > "$WORK/edges.pl" <<'EOF'
$| = 1;
system "stty raw -echo";
sub row { printf "\e[%d;1H\e[K%s", $_[0], $_[1] }
sub step { sysread STDIN, my $c, 1 }
sub mark { row(22, $_[0]); print "\e[?2026l" }
print "\e[H\e[2J";
row($_, sprintf "line %02d", $_) for 1 .. 21;
row(22, "READY");

step();
print "\e[?2026h\e[5;15r\e[2T\e[r";
mark("SD done");

step();
print "\e[?2026h\e[5;15r\e[5;1H\eM\e[r";
row(5, "RI new");
mark("RI done");

step();
print "\e[?2026h\e[5;15r\e[9;1H\e[2L\e[r";
row(9, "IL in-a"); row(10, "IL in-b");
mark("IL in done");

step();
print "\e[?2026h\e[5;15r\e[18;1H\e[2L\e[r";
row(18, "IL out-a");
mark("DONE");
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

$BIN -L "$ISOCK" respawn-pane -k -t alpha:one "perl $WORK/edges.pl; sleep 300"
wait_for '^READY'
for want in '^SD done' '^RI done' '^IL in done' '^DONE'; do
  $BIN -L "$ISOCK" send-keys -t alpha:one x
  wait_for "$want"
  echo "== painted after $want"
  $TM capture-pane -p -t client | perl -pe 's/[ \t]+$//' | head -22
done

echo "== inner grid"
$BIN -L "$ISOCK" capture-pane -p -t alpha:one | perl -pe 's/[ \t]+$//' | head -22

$BIN -L "$ISOCK" kill-server 2>/dev/null
