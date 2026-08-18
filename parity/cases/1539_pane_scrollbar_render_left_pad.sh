# Where the scrollbar's columns actually land: position left vs right, and the
# pad column, decoded cell by cell from what a client painted.
#
# screen-redraw.c:1286 splits the two positions apart: with the bar on the LEFT
# the pad follows it (cells sb_w .. sb_w+sb_pad are drawn in the pane's default
# colours), with the bar on the RIGHT the pad comes FIRST (cells 0 .. sb_pad).
# Both cases reserve width+pad columns, so the pane rectangle alone -- which is
# all case 1537 can see -- is identical either way and cannot tell a port that
# put the pad on the wrong side, or drew the bar over the pad, from a correct
# one. This case reads the painted cells instead.
#
# Same nesting as case 1538: an inner server with a client attached inside a
# pane of the outer server, so capture-pane on the outer pane returns the inner
# client's output. -N keeps trailing spaces, which the right-hand bar needs.
set -- $TM
BIN="$1"
ISOCK="sbl_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sh -c "seq 60; sleep 300"'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" set -gw pane-scrollbars on

# Decode a captured row into one character per terminal column by tracking the
# SGR state: "T" trough (the style as written, green on red), "S" slider (the
# same style with fg and bg swapped, screen-redraw.c:1274), "." anything else,
# which includes the pad cells since those take the pane's default colours.
DEC='while (<>) { chomp; my ($fg,$bg) = (0,0); my $out = "";
  while (length($_)) {
    if (s/^\e\[([0-9;]*)m//) { my $a = $1; $a = "0" if $a eq "";
      for my $c (split /;/, $a) {
        if ($c == 0)                  { ($fg,$bg) = (0,0) }
        elsif ($c == 31 || $c == 32)  { $fg = $c }
        elsif ($c == 39)              { $fg = 0 }
        elsif ($c == 41 || $c == 42)  { $bg = $c }
        elsif ($c == 49)              { $bg = 0 }
      }
      next;
    }
    next if s/^\e\[[0-9;]*[a-zA-Z]//;
    # A UTF-8 continuation byte is part of the cell before it, not a new cell:
    # the split-pane row below carries a multi-byte border glyph.
    next if s/^[\x80-\xbf]//;
    s/^(.)//s;
    $out .= ($fg == 32 && $bg == 41) ? "T" : ($fg == 31 && $bg == 42) ? "S" : ".";
  }
  print "$out\n";
}'

# Row 1 is always trough and row 16 always slider here: 23 rows of pane over 38
# lines of history gives an 8 row slider pinned to the bottom, at rows 16..23.
map() { $TM capture-pane -p -e -N -t client | sed -n '1p;16p' | perl -e "$DEC"; }
row1() { $TM capture-pane -p -e -N -t client | sed -n '1p' | perl -e "$DEC"; }

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
# Never sleep blind: poll the painted rows until they have both left the
# previous value and repeated once, so no half-finished repaint is sampled.
settle() {      # $1 = reader function, $2 = the map it must no longer read
  local read="$1" not="$2" prev="" cur="" i=0
  while [ $i -lt 200 ]; do
    cur=$($read)
    if [ "$cur" != "$not" ] && [ "$cur" = "$prev" ]; then printf '%s\n' "$cur"; return 0; fi
    prev="$cur"; i=$((i+1)); sleep 0.05
  done
  printf '%s\n<never settled>\n' "$cur"
}

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
wait_state '#{pane_width}x#{pane_height}/#{history_size}' '79x23/38'

prev=""
for spec in "left 1 0 79" "right 1 0 79" "left 2 1 77" "right 2 1 77" \
            "left 3 2 75" "right 3 2 75" "left 1 4 75" "right 1 4 75"; do
  set -- $spec
  $BIN -L "$ISOCK" set -gw pane-scrollbars-position "$1"
  $BIN -L "$ISOCK" set -gw pane-scrollbars-style "bg=red,fg=green,width=$2,pad=$3"
  wait_state '#{pane_width}x#{pane_height}/#{history_size}' "$4x23/38"
  echo "position=$1 width=$2 pad=$3 pane_width=$4"
  cur=$(settle map "$prev")
  printf '%s\n' "$cur"
  prev="$cur"
done

# Two panes side by side with DIFFERENT per-pane scrollbar widths: the map shows
# both bars and the border between them in one row, which pins that the width is
# read from each pane's own style rather than the window's.
$BIN -L "$ISOCK" set -gw pane-scrollbars-position left
$BIN -L "$ISOCK" set -gw pane-scrollbars-style 'bg=red,fg=green,width=1,pad=0'
$BIN -L "$ISOCK" split-window -h -d -t alpha:one 'sleep 300'
$BIN -L "$ISOCK" set -p -t alpha:one.0 pane-scrollbars-style 'bg=red,fg=green,width=3,pad=1'
$BIN -L "$ISOCK" set -p -t alpha:one.1 pane-scrollbars-style 'bg=red,fg=green,width=1,pad=0'
echo "split, per-pane widths 3+1 and 1+0:"
$BIN -L "$ISOCK" list-panes -t alpha:one -F '  pane #{pane_index} #{pane_left}-#{pane_right} w=#{pane_width}'
settle row1 "$(printf '%s\n' "$prev" | sed -n '1p')"

$BIN -L "$ISOCK" kill-server 2>/dev/null
