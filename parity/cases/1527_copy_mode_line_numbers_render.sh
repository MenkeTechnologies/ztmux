# copy-mode-line-numbers, as a client paints the gutter.
#
# The line-number gutter is drawn only by window_copy_write_line
# (window-copy.c:5180-5206) and exists only for an attached client. Everything
# about it is invisible server-side: the width rule (digits of
# hsize+sy+1, floored at 3, plus one for the gap -- window-copy.c:5053), the
# four numbering schemes, the two styles that separate the cursor's row from the
# rest, and the fact that the gutter pushes the pane content right by exactly
# that width.
#
# The mode also changes the POSITION INDICATOR's arithmetic, which is the part
# most likely to drift: window_copy_formats (window-copy.c:1106) reports
# #{copy_position}/#{copy_position_limit} as oy/hsize for off and default, but
# as hsize-oy+1 over hsize+sy for absolute, relative and hybrid. So "[0/8]" must
# become "[1/31]" purely by flipping the option, with no scrolling at all.
#
# Each mode is entered fresh, because the option is read while drawing and
# changing it under a live copy mode does not by itself repaint.
#
# Built like cases 1504/1507/1508: an inner server with a client attached inside
# a pane of the outer server.
set -- $TM
BIN="$1"
ISOCK="cln_$$_inner"

scrub() { perl -pe 's{/dev/tty[a-z0-9]+}{/dev/ttyDEV}g; s/\d\d:\d\d:\d\d/HH:MM:SS/g; s/\d\d:\d\d/HH:MM/g; s/[ \t]+$//'; }

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" setw -g mode-keys vi
# Distinct styles so the capture shows WHICH row got the current-line style
# rather than only that some row is styled.
$BIN -L "$ISOCK" setw -g copy-mode-line-number-style 'fg=magenta'
$BIN -L "$ISOCK" setw -g copy-mode-current-line-number-style 'fg=yellow'
$BIN -L "$ISOCK" setw -g copy-mode-position-style 'bg=blue,fg=white'

inner() { $BIN -L "$ISOCK" display-message -p -t "$1" "$2" 2>/dev/null; }
wait_for() {
  local i=0 got
  while [ $i -lt 60 ]; do
    got=$(inner "$1" "$2")
    [ "$got" = "$3" ] && return 0
    i=$((i+1)); sleep 0.05
  done
  echo "wait_for: [$2] on [$1] wanted [$3] got [$got]"
}
settle() {
  local a='' b i=0
  while [ $i -lt 30 ]; do
    b=$($TM capture-pane -p -t client)
    [ -n "$b" ] && [ "$a" = "$b" ] && return 0
    a="$b"; i=$((i+1)); sleep 0.1
  done
}
enter() {  # $1 = target window; re-enter copy mode so the option is re-read
  $BIN -L "$ISOCK" send-keys -X -t "alpha:$1" cancel 2>/dev/null
  wait_for "alpha:$1" '#{pane_in_mode}' 0
  $BIN -L "$ISOCK" copy-mode -t "alpha:$1"
  wait_for "alpha:$1" '#{pane_mode}' copy-mode
  settle
}

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
wait_for alpha '#{session_attached}' 1
wait_for alpha:one '#{pane_height}' 23
settle

# Content is made only now, at the pane's final size, so no reflow can race.
# 31 printed lines over a 23-row pane leaves 8 lines of history and the cursor
# on row 8 of the visible screen, so relative/hybrid numbering has somewhere to
# count from.
$BIN -L "$ISOCK" new-window -n data 'i=1; while [ $i -le 30 ]; do echo "line $i"; i=$((i+1)); done; sleep 300'
wait_for alpha:data '#{history_size}' 8
settle

# Row 23 of the capture is the cursor's own row (cy=22): it is the only row where
# copy-mode-current-line-number-style shows, and the only row where hybrid
# differs from relative -- hybrid prints the ABSOLUTE number there.
# The cursor is never moved while a mode is active: this case is about the
# gutter the client paints on entry, not about redraw-on-motion.
for m in off default absolute relative hybrid; do
  $BIN -L "$ISOCK" setw -g copy-mode-line-numbers "$m"
  enter data
  echo "line-numbers=$m:"
  $TM capture-pane -p -e -t client | sed -n '1,3p;23p' | cat -v | scrub
  echo "  cursor=$(inner alpha:data '#{copy_cursor_y}') position=$(inner alpha:data '#{copy_position}/#{copy_position_limit}')"
done

# Width is digits(hsize + sy + 1) + 1, floored at 4. Under 1000 total lines that
# floor is what applies (gutter "  9 "); past it the gutter has to grow, which is
# the only thing that exercises the digit loop at window-copy.c:5062.
$BIN -L "$ISOCK" setw -g copy-mode-line-numbers absolute
$BIN -L "$ISOCK" new-window -n big 'i=1; while [ $i -le 1200 ]; do echo "row $i"; i=$((i+1)); done; sleep 300'
wait_for alpha:big '#{history_size}' 1178
enter big
echo "wide gutter (1201 lines):"
$TM capture-pane -p -e -t client | sed -n '1,3p' | cat -v | scrub

$BIN -L "$ISOCK" kill-server 2>/dev/null
