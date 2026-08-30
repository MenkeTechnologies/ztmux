# The prefix key routes the NEXT key through the prefix table: with prefix set
# to C-a, C-a then a bound key runs its command, and prefix2 does the same.
set -- $TM
BIN="$1"
ISOCK="pfx_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 60 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -g prefix C-a >/dev/null
inner set -g prefix2 C-t >/dev/null
inner set -g @hit none >/dev/null
inner bind z 'set -g @hit prefix-z' >/dev/null

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

$TM send-keys -t client C-a
sleep 0.3
$TM send-keys -t client z
for _ in $(seq 1 25); do
  [ "$(inner show -gv @hit)" = prefix-z ] && break
  sleep 0.2
done
echo "after prefix C-a z: [$(inner show -gv @hit)]"

inner set -g @hit none >/dev/null
$TM send-keys -t client C-t
sleep 0.3
$TM send-keys -t client z
for _ in $(seq 1 25); do
  [ "$(inner show -gv @hit)" = prefix-z ] && break
  sleep 0.2
done
echo "after prefix2 C-t z: [$(inner show -gv @hit)]"

inner set -g @hit none >/dev/null
$TM send-keys -t client z
sleep 1
echo "without the prefix:  [$(inner show -gv @hit)]"
echo "client_prefix while idle: [$(inner list-clients -F '#{client_prefix}')]"

inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
