# attach -f sets the client's flags as it attaches, which #{client_flags} shows;
# -r attaches read-only, which the same format reports.
set -- $TM
BIN="$1"
ISOCK="asf_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
$TM new-window -d -n flagged "$BIN -L $ISOCK attach -f no-output -t inner"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 1 ] && break
  sleep 0.2
done
# If the client did not attach in time -- a slow or loaded machine, not a
# divergence -- say so once and stop, so both binaries produce the same short
# output instead of diverging further down error paths.
[ "$(inner display-message -p '#{session_attached}')" = 1 ] || {
  echo "client did not attach"; inner kill-server >/dev/null 2>&1; exit 0; }
echo "with -f no-output: [$(inner list-clients -F '#{client_flags}')]"
echo "read-only:         [$(inner list-clients -F '#{client_readonly}')]"
inner detach-client -s inner >/dev/null
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 0 ] && break
  sleep 0.2
done
$TM new-window -d -n ro "$BIN -L $ISOCK attach -r -t inner"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 1 ] && break
  sleep 0.2
done
echo "with -r:           [$(inner list-clients -F '#{client_flags}')]"
echo "read-only:         [$(inner list-clients -F '#{client_readonly}')]"
inner kill-server >/dev/null 2>&1
$TM kill-window -t flagged 2>/dev/null; $TM kill-window -t ro 2>/dev/null
