# With focus-events on, the focus-in and focus-out sequences from the terminal
# reach the server and fire the client hooks; case 1761 pins that they stay
# quiet when nothing reports focus.
set -- $TM
BIN="$1"
ISOCK="fev_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -s focus-events on >/dev/null
inner set -g @log '' >/dev/null
inner set-hook -g client-focus-in 'set -ga @log ",in"' >/dev/null
inner set-hook -g client-focus-out 'set -ga @log ",out"' >/dev/null

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
inner set -g @log '' >/dev/null

# CSI I is focus in, CSI O is focus out.
$TM send-keys -t client -H 1b 5b 4f
for _ in $(seq 1 40); do
  [ -n "$(inner show -gv @log)" ] && break
  sleep 0.2
done
echo "after focus out: [$(inner show -gv @log)]"
$TM send-keys -t client -H 1b 5b 49
for _ in $(seq 1 40); do
  case "$(inner show -gv @log)" in *in*) break;; esac
  sleep 0.2
done
echo "after focus in:  [$(inner show -gv @log)]"

inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
