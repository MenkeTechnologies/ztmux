# clock-mode draws nothing on the client's screen.
#
# The clock is painted by the CLIENT (window-clock.c's draw_screen through the
# mode's screen), not into the pane's grid: a server-side `capture-pane` is empty
# on both binaries. Read back through an attached client -- the suite's
# nested-client technique -- the reference paints the digits and this port paints
# nothing once ztmux's own floating overlay (@ztmux-ratatui) is turned off, which
# is how every other render case isolates the PORTED drawing.
#
# window_clock.rs has window_clock_draw_screen ported, so the gap is in what
# reaches the client rather than in the digits themselves. Entering and leaving
# the mode, and the options it reads, are compared by
# parity/cases/1838_clock_mode_state.sh; only the drawing is unported.
set -- $TM
BIN="$1"
ISOCK="ckg_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 10 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -g @ztmux-ratatui off >/dev/null
inner setw -g clock-mode-style 24 >/dev/null
$TM new-window -d -n client "$BIN -L $ISOCK attach -t inner"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 1 ] && break
  sleep 0.2
done
[ "$(inner display-message -p '#{session_attached}')" = 1 ] || {
  echo "client did not attach"; inner kill-server >/dev/null 2>&1; exit 0; }
inner clock-mode >/dev/null
for _ in $(seq 1 25); do
  [ -n "$(inner display-message -p '#{pane_mode}')" ] && break
  sleep 0.2
done
echo "mode: [$(inner display-message -p '#{pane_mode}')]"
echo "characters the client painted: $($TM capture-pane -p -t client | tr -d ' \n' | wc -c | tr -d ' ')"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
