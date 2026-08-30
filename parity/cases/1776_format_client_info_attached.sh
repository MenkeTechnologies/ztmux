# The same three, asked about a real client. The feature and capability answers
# come from the TERM both binaries were given, and the environment answer from
# the variable the case sets, so all three compare.
set -- $TM
BIN="$1"
ISOCK="ifm_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 6 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set-environment -g ZTPAR_ENV set-by-case >/dev/null
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
echo "features RGB:   [$(inner display-message -p -c "$c" '#{I/f:RGB}')]"
echo "features 256:   [$(inner display-message -p -c "$c" '#{I/f:256}')]"
echo "capability Ms:  [$(inner display-message -p -c "$c" '#{I/c:Ms}' | perl -pe 's/\e/ESC/g')]"
echo "environment:    [$(inner display-message -p -c "$c" '#{I/e:ZTPAR_ENV}')]"
echo "missing one:    [$(inner display-message -p -c "$c" '#{I/e:ZTPAR_NOT_SET}')]"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
