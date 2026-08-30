# confirm-before runs its command only on y; n cancels it, and -p sets the
# question. Driven through a client, since the prompt is client-side.
set -- $TM
BIN="$1"
ISOCK="cba_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 60 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -g @ran no >/dev/null
inner bind -n F1 confirm-before -p 'really? (y/n)' 'set -g @ran yes' >/dev/null

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

$TM send-keys -t client F1
sleep 0.5
$TM send-keys -t client n
sleep 1
echo "after n: [$(inner show -gv @ran)]"

$TM send-keys -t client F1
sleep 0.5
$TM send-keys -t client y
for _ in $(seq 1 25); do
  [ "$(inner show -gv @ran)" = yes ] && break
  sleep 0.2
done
echo "after y: [$(inner show -gv @ran)]"

inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
