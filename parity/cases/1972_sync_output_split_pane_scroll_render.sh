# Region scrolls inside synchronized-output blocks (DEC mode 2026) in panes that
# do NOT own the whole terminal, as an attached client paints them.
#
# 1969 scrolls a full-screen pane, where the block-end redraw lands at offset
# 0,0. Here the scrolling pane is the right half of a horizontal split (not full
# width: tty_cmd_deleteline/insertline cannot use the terminal's own scroll
# region and fall back to tty_redraw_region, tty.c:1094) and then the bottom of
# a vertical split (a non-zero yoff, so every redrawn row has to be translated
# by the pane offset). A dirty range that is too short, or a redraw that ignores
# the offset, leaves the scrolled rows stale or paints them over the neighbour.
#
# Each pane runs the same program: fill its rows, then one DL and one IL in a
# region that stops above its last row, each in its own sync block released by a
# keypress. The neighbouring pane holds fixed text that must survive untouched.
set -- $TM
BIN="$1"
ISOCK="sys_$$_inner"
WORK=$(mktemp -d)
trap '$BIN -L "$ISOCK" kill-server >/dev/null 2>&1; rm -rf "$WORK"' EXIT INT TERM

cat > "$WORK/pane.pl" <<'EOF'
$| = 1;
system "stty raw -echo";
my $tag = shift;
my ($rows) = `stty size` =~ /^(\d+)/;
my $R = $rows - 1;    # region 1..R; the last row is a fixed "statusline"
my $n = 0;
sub row { printf "\e[%d;1H\e[K%s", $_[0], $_[1] }
sub step { sysread STDIN, my $c, 1 }
print "\e[H\e[2J";
row($_, sprintf "%s %02d", $tag, ++$n) for 1 .. $R;
row($rows, "$tag READY");

step();
print "\e[?2026h\e[1;${R}r\e[1;1H\e[2M\e[r";
row($R - 2 + $_, sprintf "%s %02d", $tag, ++$n) for 1 .. 2;
row($rows, "$tag DL");
print "\e[?2026l";

step();
print "\e[?2026h\e[1;${R}r\e[1;1H\e[3L\e[r";
row($_, "$tag top$_") for 1 .. 3;
row($rows, "$tag DONE");
print "\e[?2026l";
EOF

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'echo LEFT; sleep 300'
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

# Collapse runs of spaces and box-drawing characters to <char>{xN}, as 1520
# does: the run lengths are the geometry, so a shifted column still shows.
grid() {
  $TM capture-pane -p -t client \
    | perl -CSD -pe 's/([ \x{2500}-\x{257f}])\1+/sprintf("%s{x%d}",$1,length($&))/ge; s/[ \t]+$//'
}

run_pane() {
  local target="$1" tag="$2" want
  wait_for "$tag READY"
  for want in "$tag DL" "$tag DONE"; do
    $BIN -L "$ISOCK" send-keys -t "$target" x
    wait_for "$want"
    echo "== painted after $want"
    grid
  done
}

# Right half of a horizontal split: 40 + border + 39 columns.
$BIN -L "$ISOCK" split-window -h -t alpha:one "perl $WORK/pane.pl R; sleep 300"
run_pane alpha:one.1 R

# Bottom of a vertical split inside the left pane: a pane that is neither full
# width nor at row 0.
$BIN -L "$ISOCK" split-window -v -l 10 -t alpha:one.0 "perl $WORK/pane.pl B; sleep 300"
run_pane alpha:one.1 B

echo "panes:"
$BIN -L "$ISOCK" list-panes -t alpha:one -F '#{pane_index} #{pane_width}x#{pane_height} at #{pane_left},#{pane_top}'

$BIN -L "$ISOCK" kill-server 2>/dev/null
