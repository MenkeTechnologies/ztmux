# The six popup position variables that exist only when a real MOUSE event is
# behind the menu (cmd-display-menu.c:123-127, :192-214). Case 1096 proves the
# server survives a menu taller than the screen and covers the pane/centre set;
# these six need `event->m.valid`, which no command-line invocation can produce,
# so nothing had ever expanded one:
#
#   popup_mouse_x / _y             the click, straight through
#   popup_mouse_centre_x           m.x - w/2, clamped up to 0
#   popup_mouse_centre_y           m.y - h/2, clamped so the box still fits
#   popup_mouse_top                m.y + h, or sy - 1 when that runs off
#   popup_mouse_bottom             m.y - h, clamped up to 0
#
# Each is a signed subtraction the C then clamps, and each clamp is a separate
# branch, so the case clicks once in the middle and once in each corner to take
# both sides of all four.
#
# The mouse event is injected as the bytes a terminal would send: an SGR press
# written straight into the pane the inner client is reading
# (`send-keys -H 1b 5b 3c …`), which that client's tty parser turns into a real
# mouse event. That is the only way to get one here — the outer server's own
# mouse handling is not involved.
#
# `-y` names the BOTTOM edge (cmd-display-menu.c:262), so with a 4-row box a
# `-y` of N puts the top border on row N-4. Plain-text captures only: this is
# geometry, not colour.
set -- $TM
BIN="$1"
ISOCK="pmf_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-left ''
$BIN -L "$ISOCK" set -g status-interval 0
$BIN -L "$ISOCK" set -g mouse on
# ztmux's floating overlay is an intentional extension; this case pins the PORTED
# geometry. tmux ignores the unknown user option.
$BIN -L "$ISOCK" set -g @ztmux-ratatui off

$TM new-window -d -n client "$BIN -L $ISOCK attach -t alpha"
# Wait for the inner client to be attached instead of sleeping a fixed two
# seconds: every case here must finish inside the runner's 15s budget
# (run_parity.sh:172), and a blind sleep spends an eighth of it doing nothing.
wait_client() {
  local i=0
  while [ $i -lt 300 ]; do
    [ -n "$($BIN -L "$ISOCK" list-clients -F "#{client_tty}" 2>/dev/null)" ] && { sleep 0.2; return 0; }
    i=$((i+1)); sleep 0.05
  done
  echo "wait_client: TIMEOUT"
}
wait_client

snap()   { $TM capture-pane -p -t client 2>/dev/null; }
# Settle on the BOX POSITION rather than on the whole screen: that is all this
# case reads, and two full captures per step is budget this case does not have.
settle() { local a b i=0; a=$(box); while [ $i -lt 60 ]; do sleep 0.05; b=$(box);
             [ "$a" = "$b" ] && return 0; a=$b; i=$((i+1)); done; }
box()    { snap | perl -ne 'next unless /PMT/; my ($lead) = /^( *)/;
                            printf "row=%d col=%d\n", $. - 1, length($lead); last'; }

# An SGR mouse press of the right button at 1-based (col,row), as hex bytes:
# ESC [ < 2 ; col ; row M
click() { # click <col> <row>
  local seq i=0
  seq=$(printf '\033[<2;%s;%sM' "$1" "$2" | od -An -tx1 | tr -s ' ' '\n' | grep -v '^$' | tr '\n' ' ')
  $TM send-keys -t client -H $seq
  while [ $i -lt 200 ]; do
    snap | grep -qF PMT && { settle; box; break; }
    i=$((i+1)); sleep 0.05
  done
  [ $i -lt 200 ] || echo "click: TIMEOUT at $1,$2"
  $TM send-keys -t client Escape
  i=0
  while [ $i -lt 200 ]; do
    snap | grep -qF PMT || return 0
    i=$((i+1)); sleep 0.05
  done
  echo "click: the menu would not close"
}
menu() { # menu <x-format> <y-format>
  $BIN -L "$ISOCK" bind -n MouseDown3Pane display-menu -T PMT -x "$1" -y "$2" \
    Alpha a 'list-windows' Beta b 'list-panes'
}

echo "== popup_mouse_x / popup_mouse_y: the click itself =="
menu '#{popup_mouse_x}' '#{popup_mouse_y}'
click 20 8
echo "-- a different click has to move the box with it --"
click 40 12

echo "== popup_mouse_centre_x/_y: centred on the click =="
menu '#{popup_mouse_centre_x}' '#{popup_mouse_centre_y}'
click 40 12
echo "-- near the left edge, where m.x - w/2 goes negative and clamps to 0 --"
click 2 12
echo "-- near the bottom, where the box would run off and y clamps to sy - h --"
click 40 22

$BIN -L "$ISOCK" kill-server 2>/dev/null
