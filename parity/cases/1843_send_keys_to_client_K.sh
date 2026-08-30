# send-keys -K sends the key to the CLIENT rather than to the pane, so it is
# handled as if typed at the terminal: a key bound in the root table fires.
set -- $TM
BIN="$1"
ISOCK="skk_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 6 'cat' >/dev/null
inner set -g status off >/dev/null
inner set -g @hit none >/dev/null
inner bind -n F5 'set -g @hit key-to-client' >/dev/null
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
inner send-keys -K -c "$c" F5 >/dev/null; echo "send-keys -K rc=$?"
for _ in $(seq 1 25); do
  [ "$(inner show -gv @hit)" = key-to-client ] && break
  sleep 0.2
done
echo "binding fired: [$(inner show -gv @hit)]"
echo "== without -K the key goes to the pane instead =="
inner set -g @hit none >/dev/null
inner send-keys F5 >/dev/null
sleep 0.5
echo "binding fired: [$(inner show -gv @hit)]"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
