# The copy-mode SELECTION as a client paints it, driven by real vi keys.
#
# Server-side formats already pin the selection COORDINATES (#{selection_present},
# #{copy_cursor_x/y}, #{rectangle_toggle}). What no server-side case can see is
# the painted result: window_copy_update_style (window-copy.c:4940) decides, cell
# by cell, whether a cell is inside the selection and swaps in
# copy-mode-selection-style, and that only runs while a client is drawing.
#
# The three shapes differ in a way only the screen shows. A normal selection is a
# stream: from the anchor it runs to the END of every intermediate line -- past
# the text, over the padding -- and stops at the cursor column on the last line.
# A rectangle clamps every row to the same two columns and paints the padding of
# short lines (it is also the one shape where the cursor may sit past end of
# line, window-copy.c:5424). select-line ignores columns and takes whole lines.
# A port that tracked the coordinates correctly but built each row's highlight
# from the cursor column alone would pass every existing case and fail here.
#
# Keys go through the client, not `send-keys -X`, so the copy-mode-vi table is
# part of what is pinned: Space=begin-selection, l/j/k=cursor-right/down/up,
# v=rectangle-toggle, V=select-line, Escape=clear-selection (key-bindings.c:597+).
#
# Built like cases 1504/1507/1508: an inner server with a client attached inside
# a pane of the outer server.
set -- $TM
BIN="$1"
ISOCK="csr_$$_inner"

scrub() { perl -pe 's{/dev/tty[a-z0-9]+}{/dev/ttyDEV}g; s/\d\d:\d\d:\d\d/HH:MM:SS/g; s/\d\d:\d\d/HH:MM/g; s/[ \t]+$//'; }

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-interval 0
# ztmux's floating overlay is an intentional extension; this case pins the
# PORTED rendering, so disable it. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off
$BIN -L "$ISOCK" setw -g mode-keys vi
# mode-style and copy-mode-selection-style are deliberately DIFFERENT: the
# selection must come out green, which proves the selection option is what is
# applied and not the mode-style it defaults to.
$BIN -L "$ISOCK" setw -g mode-style 'bg=cyan,fg=black'
$BIN -L "$ISOCK" setw -g copy-mode-selection-style 'bg=green,fg=black'
# Silence the position indicator: case 1524 owns it, and it would overwrite the
# right-hand end of the row 0 highlight that this case is about.
$BIN -L "$ISOCK" setw -g copy-mode-position-format ''

# Bounded polls (~3s each) instead of blind sleeps; the whole case runs in about
# two and a half seconds, well inside the runner's 15s per-case budget.
inner() { $BIN -L "$ISOCK" display-message -p -t "$1" "$2" 2>/dev/null; }
wait_for() {
  local i=0 got
  while [ $i -lt 60 ]; do
    got=$(inner "$1" "$2")
    [ "$got" = "$3" ] && return 0
    i=$(( i + 1 )); sleep 0.05
  done
  echo "wait_for: [$2] on [$1] wanted [$3] got [$got]"
}
settle() {
  local a='' b i=0
  while [ $i -lt 30 ]; do
    b=$($TM capture-pane -p -t client)
    [ -n "$b" ] && [ "$a" = "$b" ] && return 0
    a="$b"; i=$(( i + 1 )); sleep 0.1
  done
}
screen() { $TM capture-pane -p -e -t client | sed -n '1,4p' | cat -v | scrub; }
coords() { echo "coords: $(inner alpha:data 'sel=#{selection_present} rect=#{rectangle_toggle} cx=#{copy_cursor_x} cy=#{copy_cursor_y}')"; }
key() { $TM send-keys -t client "$@"; }

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
wait_for alpha '#{session_attached}' 1
wait_for alpha:one '#{pane_height}' 23
settle

# Lines of deliberately different lengths: the rectangle has to paint the
# padding past the end of "cc" and "dddddd".
$BIN -L "$ISOCK" new-window -n data 'printf "aaaaaaaa\nbbbbbbbbbb\ncc\ndddddd\n"; sleep 300'
wait_for alpha:data '#{pane_at_bottom}' 1
settle

key C-b; key '['
wait_for alpha:data '#{pane_mode}' copy-mode
key g; wait_for alpha:data '#{copy_cursor_y}' 0
key 0; wait_for alpha:data '#{copy_cursor_x}' 0
settle
echo "no selection:"; screen

# begin-selection only materialises the selection on the first move, so the wait
# here is on the cursor after the moves, not on selection_present.
key Space
key l; key l; key l
wait_for alpha:data '#{copy_cursor_x}' 3
key j
wait_for alpha:data '#{copy_cursor_y}' 1
settle
echo "stream selection (0,0)-(3,1):"; screen
coords

key v
wait_for alpha:data '#{rectangle_toggle}' 1
settle
echo "rectangle (0,0)-(3,1):"; screen

# In a rectangle the cursor keeps column 3 even over the 2-column line and the
# padding under it is painted; outside one it would have been clamped.
key j; key j
wait_for alpha:data '#{copy_cursor_y}' 3
settle
echo "rectangle down over short lines:"; screen
coords

# select-line drops the rectangle and takes whole lines, then extends by line.
key V
wait_for alpha:data '#{rectangle_toggle}' 0
settle
echo "select-line:"; screen
key k
wait_for alpha:data '#{copy_cursor_y}' 2
settle
echo "select-line extended up:"; screen
coords

key Escape
wait_for alpha:data '#{selection_present}' 0
settle
echo "clear-selection:"; screen

$BIN -L "$ISOCK" kill-server 2>/dev/null
