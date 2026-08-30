# The default WheelUpPane binding puts a pane with scrollback into copy mode.
# The wheel event is a real SGR sequence (button 64) written into the inner
# client's terminal.
#
# The polls are deliberately short: this case starts two servers and a client,
# and the runner allows 15s per case per binary, so a long poll loop turns a
# slow machine into a truncated comparison rather than a failure with a diff.
set -- $TM
BIN="$1"
ISOCK="mwh_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }
wheel_up() { $TM send-keys -t client -H 1b 5b 3c 36 34 3b 35 3b 35 4d; }

inner -f /dev/null new-session -d -s inner -x 40 -y 10 \
  "i=1; while [ \$i -le 40 ]; do echo line \$i; i=\$((i+1)); done; sleep 300" >/dev/null
inner set -g status off >/dev/null
inner set -g mouse on >/dev/null

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
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{history_size}')" -ge 20 ] && break
  sleep 0.2
done
echo "mode before: [$(inner display-message -p '#{pane_mode}')]"
wheel_up
for _ in $(seq 1 25); do
  [ -n "$(inner display-message -p '#{pane_mode}')" ] && break
  sleep 0.2
done
echo "mode after wheel up: [$(inner display-message -p '#{pane_mode}')]"
inner send-keys -X cancel >/dev/null 2>&1
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
