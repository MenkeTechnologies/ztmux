# A control-mode client (-C) answers each command with a %begin/%end block and
# reports events as % lines. The client runs in a pane of the outer server, so
# capture-pane reads exactly what it printed. The ids and timestamps in the
# block headers are masked; the structure and the payload are what is compared.
set -- $TM
BIN="$1"
ISOCK="ctl_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }
scrub() { perl -pe 's/^(%(?:begin|end|error)) \d+ \d+ \d+$/$1 T N F/; s/\s+$//'; }

inner -f /dev/null new-session -d -s inner -n one -x 40 -y 6 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -g automatic-rename off >/dev/null

$TM new-window -d -n ctl "$BIN -L $ISOCK -C attach -t inner"
for _ in $(seq 1 60); do
  [ "$(inner display-message -p '#{session_attached}')" = 1 ] && break
  sleep 0.2
done
# If the client did not attach in time -- a slow or loaded machine, not a
# divergence -- say so once and stop, so both binaries produce the same short
# output instead of diverging further down error paths.
[ "$(inner display-message -p '#{session_attached}')" = 1 ] || {
  echo "client did not attach"; inner kill-server >/dev/null 2>&1; exit 0; }
echo "control client: $(inner list-clients -F 'control_mode=#{client_control_mode} flags=#{client_flags}')"

$TM send-keys -t ctl -l 'display-message -p hello-control'
$TM send-keys -t ctl Enter
for _ in $(seq 1 60); do
  [ "$($TM capture-pane -p -t ctl | grep -c hello-control)" -ge 1 ] && break
  sleep 0.2
done
echo "reply block:"
$TM capture-pane -p -t ctl | scrub | grep -v '^$' | tail -4

$TM send-keys -t ctl -l 'nosuchcommand'
$TM send-keys -t ctl Enter
for _ in $(seq 1 60); do
  [ "$($TM capture-pane -p -t ctl | grep -c 'unknown command')" -ge 1 ] && break
  sleep 0.2
done
echo "error block:"
$TM capture-pane -p -t ctl | scrub | grep -v '^$' | tail -3

inner kill-server >/dev/null 2>&1
$TM kill-window -t ctl 2>/dev/null
