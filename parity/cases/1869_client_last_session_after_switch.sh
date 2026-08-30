# #{client_last_session} names the session a client was on before the current
# one, which switch-client sets as it moves.
set -- $TM
BIN="$1"
ISOCK="cls_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s first -x 40 -y 8 'sleep 300' >/dev/null
inner new-session -d -s second -x 40 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
$TM new-window -d -n client "$BIN -L $ISOCK attach -t first"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p -t first '#{session_attached}')" = 1 ] && break
  sleep 0.2
done
# If the client did not attach in time -- a slow or loaded machine, not a
# divergence -- say so once and stop, so both binaries produce the same short
# output instead of diverging further down error paths.
[ "$(inner display-message -p '#{session_attached}')" = 1 ] || {
  echo "client did not attach"; inner kill-server >/dev/null 2>&1; exit 0; }
c=$(inner list-clients -F '#{client_name}' | head -1)
echo "session: [$(inner list-clients -F '#{client_session}')] last: [$(inner list-clients -F '#{client_last_session}')]"
inner switch-client -c "$c" -t second >/dev/null
for _ in $(seq 1 25); do
  [ "$(inner list-clients -F '#{client_session}')" = second ] && break
  sleep 0.2
done
echo "after switching: session: [$(inner list-clients -F '#{client_session}')] last: [$(inner list-clients -F '#{client_last_session}')]"
inner switch-client -c "$c" -l >/dev/null
for _ in $(seq 1 25); do
  [ "$(inner list-clients -F '#{client_session}')" = first ] && break
  sleep 0.2
done
echo "after -l:        session: [$(inner list-clients -F '#{client_session}')] last: [$(inner list-clients -F '#{client_last_session}')]"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
