# The popup position variables that only exist when a CLIENT with a status line
# is asking (cmd-display-menu.c:125-177). Case 1096 covers the four
# `popup_pane_*` and the two `popup_centre_*`, which the server can compute with
# no client; these three cannot be reached that way and no case had expanded one:
#
#   popup_width / popup_height         the menu's own size, so `-x #{popup_width}`
#                                      lands the box exactly its own width in
#   popup_status_line_y                lines + h with the status at the top,
#                                      tty->sy - lines at the bottom
#
# The two `popup_window_status_line_*` names come from the same block and are
# case 1964: they depend on the status RANGES rather than on the geometry here,
# and splitting them keeps both cases inside the runner's 15s budget.
#
# They are readable only as a POSITION, because nothing else expands them: menu
# item text goes through a fresh format tree (`format_single_from_state`,
# menu.c:89-92), so the value has to be read off where the box actually landed.
# That is the behaviour that matters anyway — these exist to place a menu against
# the status line, and the arithmetic differs per status position and per number
# of status lines, which is what the passes below walk.
#
# `-y` names the BOTTOM edge (cmd-display-menu.c:262, `n -= h`), so a box whose
# height equals the value starts at row 0. The captures are plain text, never
# `-e`: this case is about geometry, so it stays out of the theme-colour
# divergence that keeps the render cases quarantined.
#
# Built like 1540: an inner server with an attached client in a pane of the outer
# one, so capture-pane on the OUTER server reads back what the inner client drew.
set -- $TM
BIN="$1"
ISOCK="ppf_$$_inner"

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

echo "== popup_width and popup_height place the box by its own size =="
open_menu '#{popup_width}' '#{popup_height}'

echo "== the same box one column further in, to prove the column is read =="
open_menu '#{e|+|:#{popup_width},1}' '#{popup_height}'

echo "== popup_status_line_y, status at the bottom (the default) =="
open_menu 0 '#{popup_status_line_y}'

echo "== popup_status_line_y, status at the top =="
$BIN -L "$ISOCK" set -g status-position top
open_menu 0 '#{popup_status_line_y}'

echo "== and with two status lines, which changes the line count =="
$BIN -L "$ISOCK" set -g status 2
open_menu 0 '#{popup_status_line_y}'
$BIN -L "$ISOCK" kill-server 2>/dev/null
