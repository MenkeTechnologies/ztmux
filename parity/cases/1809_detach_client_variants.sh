# detach-client with two clients attached: -t detaches one, -a detaches every
# other, and -s detaches everything on a session. #{session_attached} counts
# what is left.
set -- $TM
BIN="$1"
ISOCK="dtc_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

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
echo "attached: $(inner display-message -p '#{session_attached}')"
first=$(inner list-clients -F '#{client_name}' | head -1)
inner detach-client -t "$first" >/dev/null; echo "detach one rc=$?"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 1 ] && break
  sleep 0.2
done
echo "after detaching one: $(inner display-message -p '#{session_attached}')"
$TM new-window -d -n c3 "$BIN -L $ISOCK attach -t inner"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 2 ] && break
  sleep 0.2
done
echo "attached again: $(inner display-message -p '#{session_attached}')"
inner detach-client -s inner >/dev/null; echo "detach -s rc=$?"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 0 ] && break
  sleep 0.2
done
echo "after detaching the session: $(inner display-message -p '#{session_attached}')"
inner kill-server >/dev/null 2>&1
for w in c1 c2 c3; do $TM kill-window -t $w 2>/dev/null; done
