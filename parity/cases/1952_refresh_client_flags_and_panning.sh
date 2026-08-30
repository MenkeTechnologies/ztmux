# refresh-client against a real (non-control) client: -F/-f set the client flags,
# -L/-R/-U/-D pan the client's view by the ADJUSTMENT given as an argument
# (cmd-refresh-client.c:213-257), and -A/-B/-C are refused for a client that is
# not in control mode. The client comes from an inner server attached inside a
# pane of the outer one, the only way this suite gets a client at all.
set -- $TM
BIN="$1"
ISOCK="rcf_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
$TM new-window -d -n client "$BIN -L $ISOCK attach -t inner"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 1 ] && break
  sleep 0.2
done
[ "$(inner display-message -p '#{session_attached}')" = 1 ] || {
  echo "client did not attach"; inner kill-server >/dev/null 2>&1; exit 0; }
c=$(inner list-clients -F '#{client_name}' | head -1)
echo "flags at first: [$(inner display-message -p -t "$c" '#{client_flags}')]"
# The control-only flag names are read only for a control client
# (server-client.c:2864-2866), so no-output is silently ignored here.
inner refresh-client -t "$c" -F no-output; echo "-F no-output rc=$?"
echo "unchanged:      [$(inner display-message -p -t "$c" '#{client_flags}')]"
inner refresh-client -t "$c" -f ignore-size; echo "-f ignore-size rc=$?"
echo "flags now:      [$(inner display-message -p -t "$c" '#{client_flags}')]"
inner refresh-client -t "$c" -F active-pane,no-detach-on-destroy; echo "-F two rc=$?"
echo "flags now:      [$(inner display-message -p -t "$c" '#{client_flags}')]"
inner refresh-client -t "$c" -f '!active-pane'; echo "clearing one rc=$?"
echo "flags now:      [$(inner display-message -p -t "$c" '#{client_flags}')]"
echo "== read-only goes on and, by the C's own guard, does not come off =="
inner refresh-client -t "$c" -f read-only; echo "rc=$?"
echo "flags now:      [$(inner display-message -p -t "$c" '#{client_flags}')]"
inner refresh-client -t "$c" -f '!read-only'; echo "rc=$?"
echo "flags still:    [$(inner display-message -p -t "$c" '#{client_flags}')]"
echo "== an unknown flag name is ignored =="
inner refresh-client -t "$c" -f nosuchclientflag; echo "rc=$?"
echo "flags still:    [$(inner display-message -p -t "$c" '#{client_flags}')]"
echo "== panning: the window is bigger than the client =="
inner set -w -t inner:0 window-size manual >/dev/null
inner resize-window -t inner:0 -x 80 -y 20 >/dev/null
inner refresh-client -t "$c" -D 3; echo "-D 3 rc=$?"
inner refresh-client -t "$c" -R 5; echo "-R 5 rc=$?"
inner refresh-client -t "$c" -U;   echo "-U rc=$?"
inner refresh-client -t "$c" -L;   echo "-L rc=$?"
inner refresh-client -t "$c" -D notanumber; echo "rc=$?"
inner refresh-client -t "$c" -D 0; echo "rc=$?"
echo "== -A, -B and -C want a control client =="
inner refresh-client -t "$c" -A '%1:on'; echo "-A rc=$?"
inner refresh-client -t "$c" -B 'sub:%1:#{pane_id}'; echo "-B rc=$?"
inner refresh-client -t "$c" -C 40x10; echo "-C rc=$?"
echo "== -l asks for the clipboard, which is silent here =="
inner refresh-client -t "$c" -l; echo "-l rc=$?"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
