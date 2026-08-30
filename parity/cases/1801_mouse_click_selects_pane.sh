# With the mouse on, a click in a pane makes it the active one. The click is a
# real SGR mouse sequence written into the inner client's terminal (the outer
# pane), which is the only way to drive the mouse without a mouse. The rows are
# taken from the panes' own geometry rather than assumed.
set -- $TM
BIN="$1"
ISOCK="mcl_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }
hex() { printf '%s' "$1" | perl -ne 'print join(" ", map { sprintf "%02x", ord } split //)'; }
# SGR press-and-release of button 0 at column $1, row $2 (both 1-based).
click() {
  $TM send-keys -t client -H 1b 5b 3c 30 3b $(hex "$1") 3b $(hex "$2") 4d
  $TM send-keys -t client -H 1b 5b 3c 30 3b $(hex "$1") 3b $(hex "$2") 6d
}

inner -f /dev/null new-session -d -s inner -x 40 -y 12 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -g mouse on >/dev/null
inner split-window -d -t inner 'sleep 300' >/dev/null

$TM new-window -d -n client "$BIN -L $ISOCK attach -t inner"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 1 ] && break
  sleep 0.2
done
# If the client did not attach in time -- a slow or loaded machine, not a
# divergence -- say so once and stop, so both binaries produce the same short
# output instead of diverging further down error paths.
[ "$(inner display-message -p '#{session_attached}')" = 1 ] || {
  echo "client did not attach"; inner kill-server >/dev/null 2>&1; exit 0; }
inner select-pane -t 0 >/dev/null
echo "panes: $(inner list-panes -F '#{pane_index}@#{pane_top}+#{pane_height}' | tr '\n' ' ')"
lower_row=$(( $(inner display-message -p -t inner.1 '#{pane_top}') + 2 ))
upper_row=$(( $(inner display-message -p -t inner.0 '#{pane_top}') + 2 ))
echo "active before: $(inner display-message -p '#{pane_index}')"

click 5 "$lower_row"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{pane_index}')" = 1 ] && break
  sleep 0.2
done
echo "active after clicking the lower pane: $(inner display-message -p '#{pane_index}')"

click 5 "$upper_row"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{pane_index}')" = 0 ] && break
  sleep 0.2
done
echo "active after clicking the upper pane: $(inner display-message -p '#{pane_index}')"

inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
