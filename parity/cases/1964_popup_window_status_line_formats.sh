# `popup_window_status_line_x` and `popup_window_status_line_y`: where THIS
# window's entry sits in the status line, which a menu can be hung off
# (cmd-display-menu.c:139-163). The C walks every status line's ranges for a
# STYLE_RANGE_WINDOW whose argument is the winlink index, and only adds the two
# variables if it finds one -- so a port that builds the window list without
# recording its ranges leaves both unset and every menu lands at 0,0.
#
# The row is computed one way with the status at the top (line + 1 + h) and the
# other way at the bottom (sy - lines + line), and the column follows the entry,
# so `status-justify centre` has to move it. Case 1961 covers the rest of the
# client-side popup set; this is the half that depends on the status ranges.
#
# `-y` names the BOTTOM edge (cmd-display-menu.c:262). Plain-text captures only:
# geometry, not colour.
set -- $TM
BIN="$1"
ISOCK="pwf_$$_inner"

$BIN -L "$ISOCK" -f /dev/null new-session -d -s alpha -n one -x 80 -y 24 'sleep 300'
$BIN -L "$ISOCK" set -g status-right ''
$BIN -L "$ISOCK" set -g status-left ''
$BIN -L "$ISOCK" set -g status-interval 0
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

snap()   { $TM capture-pane -p -t client; }
# Settle on the BOX POSITION rather than on the whole screen: that is all this
# case reads, and comparing two full 24-row captures per step costs more of the
# runner's 15s budget than it buys.
settle() { local a b i=0; a=$(box); while [ $i -lt 60 ]; do sleep 0.05; b=$(box);
             [ "$a" = "$b" ] && return 0; a=$b; i=$((i+1)); done; }
# Where the box landed: the row the title is on and the column its left edge
# starts at. Both are read off the drawn screen, so they are the values the
# formats produced and not a restatement of them.
box() {
  snap | perl -ne 'next unless /PMT/; my ($lead) = /^( *)/;
                   printf "row=%d col=%d\n", $. - 1, length($lead); last'
}
open_menu() { # open_menu <x-format> <y-format>
  local i=0
  $BIN -L "$ISOCK" bind M display-menu -T PMT -x "$1" -y "$2" \
    Alpha a 'list-windows' Beta b 'list-panes'
  $TM send-keys -t client C-b; sleep 0.15; $TM send-keys -t client M
  while [ $i -lt 200 ]; do
    snap | grep -qF PMT && { settle; box; break; }
    i=$((i+1)); sleep 0.05
  done
  [ $i -lt 200 ] || echo "open_menu TIMEOUT [$1][$2]"
  $TM send-keys -t client Escape
  i=0
  while [ $i -lt 200 ]; do
    snap | grep -qF PMT || return 0
    i=$((i+1)); sleep 0.05
  done
  echo "open_menu: the menu would not close"
}

echo "== popup_window_status_line_x/_y: this window's entry in the status line =="
open_menu '#{popup_window_status_line_x}' '#{popup_window_status_line_y}'

echo "== centred status: the window entry moves, so the column must move with it =="
$BIN -L "$ISOCK" set -g status-justify centre
open_menu '#{popup_window_status_line_x}' '#{popup_window_status_line_y}'

echo "== and with the status at the top, where the row is computed the other way =="
$BIN -L "$ISOCK" set -g status-position top
open_menu '#{popup_window_status_line_x}' '#{popup_window_status_line_y}'

echo "== the status line the formats were read against =="
snap | perl -ne 'print if $. == 1' | cat -v | perl -pe 's/[ \t]+$//'

$BIN -L "$ISOCK" kill-server 2>/dev/null
