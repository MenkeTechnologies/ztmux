# display-menu with a client: pressing an item's key runs that item's command,
# and Escape closes the menu without running anything. The render is pinned
# elsewhere (1541); this pins the choosing.
set -- $TM
BIN="$1"
ISOCK="mnu_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 60 -y 10 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -g @chosen none >/dev/null
inner bind -n F1 display-menu -x 0 -y 0 -T menu \
  'first'  f 'set -g @chosen first' \
  'second' s 'set -g @chosen second' >/dev/null

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
sleep 0.6
$TM send-keys -t client s
for _ in $(seq 1 25); do
  [ "$(inner show -gv @chosen)" = second ] && break
  sleep 0.2
done
echo "after pressing s: [$(inner show -gv @chosen)]"

inner set -g @chosen none >/dev/null
$TM send-keys -t client F1
sleep 0.6
$TM send-keys -t client Escape
sleep 1
echo "after Escape:     [$(inner show -gv @chosen)]"

inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
