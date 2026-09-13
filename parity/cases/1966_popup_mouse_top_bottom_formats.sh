# `popup_mouse_top` and `popup_mouse_bottom`, the two mouse-relative positions
# case 1963 does not cover (cmd-display-menu.c:204-213). Both are a signed
# arithmetic on the click row with a clamp on one side only:
#
#   popup_mouse_top      m.y + h, or sy - 1 when the box would run off the foot
#   popup_mouse_bottom   m.y - h, clamped up to 0 when the click is within one
#                        box height of the top
#
# The clamped branch of `bottom` is the interesting one: the C computes it as a
# long and then hands an unsigned 0 to a `-y` that names the BOTTOM edge, so the
# box ends up placed from a subtraction that has already gone negative once.
#
# The mouse event is injected as the bytes a terminal sends -- an SGR press
# written into the pane the inner client reads -- because nothing on the command
# line can produce an event with m.valid set. Split from 1963 so both stay inside
# the runner's 15s budget.
set -- $TM
BIN="$1"
ISOCK="pmt_$$_inner"

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

echo "== popup_mouse_top: m.y + h, or sy - 1 when that would not fit =="
menu 0 '#{popup_mouse_top}'
click 40 4
echo "-- clicking low enough that m.y + h runs past the screen --"
click 40 22

echo "== popup_mouse_bottom: m.y - h, clamped up to 0 =="
menu 0 '#{popup_mouse_bottom}'
click 40 12
echo "-- clicking above the box height, where the subtraction goes negative --"
click 40 2

$BIN -L "$ISOCK" kill-server 2>/dev/null
