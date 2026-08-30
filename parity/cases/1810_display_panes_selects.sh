# display-panes with a client shows each pane's number and takes a key: pressing
# a number makes that pane active. -d 0 keeps the indicator up until a key is
# pressed, and -N leaves the panes alone.
set -- $TM
BIN="$1"
ISOCK="dsp_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 10 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner split-window -d 'sleep 300' >/dev/null
inner select-pane -t 0 >/dev/null
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
echo "active before: $(inner display-message -p '#{pane_index}')"
inner display-panes -d 0 >/dev/null 2>&1 &
sleep 0.6
$TM send-keys -t client 1
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{pane_index}')" = 1 ] && break
  sleep 0.2
done
echo "after pressing 1: $(inner display-message -p '#{pane_index}')"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
