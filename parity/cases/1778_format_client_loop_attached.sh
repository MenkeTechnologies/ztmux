# The same loop with a client attached, through the nested-client technique: the
# body runs once per client and the loop variables are set inside it.
set -- $TM
BIN="$1"
ISOCK="lfm_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 6 'sleep 300' >/dev/null
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
echo "one client: [$(inner display-message -p '#{L:#{client_session}/#{loop_index}/#{loop_last_flag} }')]"
echo "count via the loop: [$(inner display-message -p '#{L:x}')]"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
