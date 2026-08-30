# detach-client -E runs a shell command instead of the client's normal detach
# path, so the pane the client was in shows the command's output rather than
# ending.
set -- $TM
BIN="$1"
ISOCK="dce_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
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
c=$(inner list-clients -F '#{client_name}' | head -1)
inner detach-client -E "printf 'DETACH-COMMAND-RAN\n'; sleep 300" -t "$c" >/dev/null; echo "rc=$?"
for _ in $(seq 1 25); do
  [ "$($TM capture-pane -p -t client | grep -c DETACH-COMMAND-RAN)" -ge 1 ] && break
  sleep 0.2
done
echo "the command ran in the client's pane: $($TM capture-pane -p -t client | grep -c DETACH-COMMAND-RAN)"
echo "session now attached: $(inner display-message -p '#{session_attached}')"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
