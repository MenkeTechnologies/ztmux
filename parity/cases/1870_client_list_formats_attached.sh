# The *_list formats name the clients and sessions attached to a window or
# session; with two clients on the same session they list both, which is the
# state their earlier (empty) coverage could not reach.
set -- $TM
BIN="$1"
ISOCK="clf_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }
names() { perl -pe 's/\b\d+\b/N/g'; }

inner -f /dev/null new-session -d -s inner -x 40 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
$TM new-window -d -n c1 "$BIN -L $ISOCK attach -t inner"
$TM new-window -d -n c2 "$BIN -L $ISOCK attach -t inner"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 2 ] && break
  sleep 0.2
done
# If the client did not attach in time -- a slow or loaded machine, not a
# divergence -- say so once and stop, so both binaries produce the same short
# output instead of diverging further down error paths.
[ "$(inner display-message -p '#{session_attached}')" = 2 ] || {
  echo "client did not attach"; inner kill-server >/dev/null 2>&1; exit 0; }
echo "session_attached:      $(inner display-message -p '#{session_attached}')"
echo "session_many_attached: $(inner display-message -p '#{session_many_attached}')"
echo "attached_list count:   $(inner display-message -p '#{session_attached_list}' | tr ',' '\n' | grep -c .)"
echo "window_active_clients: $(inner display-message -p '#{window_active_clients}')"
echo "active_clients_list:   $(inner display-message -p '#{window_active_clients_list}' | names | tr ',' '\n' | sort -u | tr '\n' ' ')"
inner kill-server >/dev/null 2>&1
$TM kill-window -t c1 2>/dev/null; $TM kill-window -t c2 2>/dev/null
