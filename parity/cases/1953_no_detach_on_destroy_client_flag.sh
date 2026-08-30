# The no-detach-on-destroy client flag: with detach-on-destroy "on" (the
# default) killing a client's session normally detaches that client, because
# server_destroy_session picks no replacement. A client carrying this flag is
# moved to an alternative session instead (server-fn.c:456-470).
set -- $TM
BIN="$1"
ISOCK="ndod_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s doomed -x 40 -y 8 'sleep 300' >/dev/null
inner new-session -d -s survivor -x 40 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
echo "detach-on-destroy: $(inner show -gv detach-on-destroy)"
$TM new-window -d -n client "$BIN -L $ISOCK attach -t doomed"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p -t doomed '#{session_attached}')" = 1 ] && break
  sleep 0.2
done
[ "$(inner display-message -p -t doomed '#{session_attached}')" = 1 ] || {
  echo "client did not attach"; inner kill-server >/dev/null 2>&1; exit 0; }
c=$(inner list-clients -F '#{client_name}' | head -1)
echo "client session: $(inner display-message -p -t "$c" '#{session_name}')"
inner refresh-client -t "$c" -f no-detach-on-destroy; echo "flag rc=$?"
echo "flags: [$(inner display-message -p -t "$c" '#{client_flags}')]"
inner kill-session -t doomed; echo "kill rc=$?"
for _ in $(seq 1 25); do
  [ "$(inner list-clients | wc -l | tr -d ' ')" = 1 ] && \
    [ -n "$(inner display-message -p -t "$c" '#{session_name}' 2>/dev/null)" ] && break
  sleep 0.2
done
echo "clients left: $(inner list-clients | wc -l | tr -d ' ')"
echo "client moved to: $(inner display-message -p -t "$c" '#{session_name}')"
echo "sessions: $(inner list-sessions -F '#{session_name}' | tr '\n' ' ')"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
