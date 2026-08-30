# choose-tree with a client: moving the selection and pressing Enter switches to
# the chosen window. The drawing is pinned by 1508; this pins the choice.
set -- $TM
BIN="$1"
ISOCK="cts_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -n first -x 60 -y 10 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -g automatic-rename off >/dev/null
inner new-window -d -n second 'sleep 300' >/dev/null
inner new-window -d -n third 'sleep 300' >/dev/null
inner select-window -t first >/dev/null

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
echo "current window before: [$(inner display-message -p '#{window_name}')]"

inner choose-tree -w >/dev/null
for _ in $(seq 1 25); do
  [ -n "$(inner display-message -p '#{pane_mode}')" ] && break
  sleep 0.2
done
echo "mode: [$(inner display-message -p '#{pane_mode}')]"
$TM send-keys -t client Down
sleep 0.4
$TM send-keys -t client Enter
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{window_name}')" != first ] && break
  sleep 0.2
done
echo "current window after:  [$(inner display-message -p '#{window_name}')]"
echo "mode after choosing:   [$(inner display-message -p '#{pane_mode}')]"

inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
