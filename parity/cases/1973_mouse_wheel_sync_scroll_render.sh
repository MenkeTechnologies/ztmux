# Mouse-wheel scrolling of an application that scrolls itself, end to end, as an
# attached client paints it -- the path nvim takes.
#
# With `mouse on` and an application that asked for mouse reporting (SGR, mode
# 1006), a wheel event on the client's terminal is a WheelUpPane/WheelDownPane
# key that is forwarded to the pane as an SGR report (input_key_get_mouse). The
# application answers the way nvim does: inside a synchronized-output block it
# sets a scroll region above its statusline, deletes (wheel down) or inserts
# (wheel up) three lines, repaints the rows that scrolled in, and ends the
# block. What the client paints depends on every link of that chain -- mouse
# parsing on the client tty, the forwarded report, the region scroll, and the
# dirty rows redrawn at the end of the sync block.
#
# The wheel bytes are typed into the OUTER pane that runs the inner client, so
# they arrive on the inner client's tty exactly as a terminal would send them.
# The program's statusline reports the top line after each event; each wait
# polls the painted screen for it, so every event is its own read.
set -- $TM
BIN="$1"
ISOCK="syw_$$_inner"
WORK=$(mktemp -d)
trap '$BIN -L "$ISOCK" kill-server >/dev/null 2>&1; rm -rf "$WORK"' EXIT INT TERM

cat > "$WORK/wheel.pl" <<'EOF'
$| = 1;
system "stty raw -echo";
my $R = 21;    # text rows 1..21; row 22 is the statusline
my $top = 1;
sub row { printf "\e[%d;1H\e[K%s", $_[0], $_[1] }
sub text { sprintf "text %03d", $_[0] }
sub status { row($R + 1, sprintf "top %03d", $top) }
print "\e[?1000h\e[?1006h\e[H\e[2J";
row($_, text($_)) for 1 .. $R;
status();
my $buf = '';
while (sysread STDIN, my $c, 64) {
    $buf .= $c;
    while ($buf =~ s/\e\[<(\d+);\d+;\d+[Mm]//) {
        my $b = $1;
        print "\e[?2026h\e[1;${R}r\e[1;1H";
        if ($b == 65) {
            $top += 3;
            print "\e[3M\e[r";
            row($_, text($top + $_ - 1)) for $R - 2 .. $R;
        } elsif ($b == 64 && $top > 1) {
            $top -= 3;
            print "\e[3L\e[r";
            row($_, text($top + $_ - 1)) for 1 .. 3;
        } else {
            print "\e[r";
        }
        status();
        print "\e[?2026l";
    }
}
EOF

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status off
$BIN -L "$ISOCK" set -g mouse on
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

# One SGR wheel report (64 up, 65 down) at column 20, row 10 of the client.
wheel() { $TM send-keys -t client -l "$(printf '\033[<%d;20;10M' "$1")"; }

$BIN -L "$ISOCK" respawn-pane -k -t alpha:one "perl $WORK/wheel.pl"
wait_for '^top 001'
echo "mouse_any_flag: $($BIN -L "$ISOCK" display-message -p -t alpha:one '#{mouse_any_flag} #{mouse_sgr_flag}')"

top=1
for b in 65 65 65 65 64 65 64 64; do
  if [ "$b" = 65 ]; then top=$((top+3)); else top=$((top-3)); fi
  wheel "$b"
  wait_for "$(printf '^top %03d' "$top")"
done
echo "== painted after the wheel sequence"
$TM capture-pane -p -t client | perl -pe 's/[ \t]+$//' | head -22

echo "== inner grid"
$BIN -L "$ISOCK" capture-pane -p -t alpha:one | perl -pe 's/[ \t]+$//' | head -22

$BIN -L "$ISOCK" kill-server 2>/dev/null
