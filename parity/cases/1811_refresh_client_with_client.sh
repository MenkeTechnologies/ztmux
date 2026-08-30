# refresh-client with a client attached: -f sets the client flags that
# #{client_flags} reads back, -C resizes the client, and -S redraws the status
# only. Polls are short: this case starts two servers (see the timing note in
# the roadmap).
set -- $TM
BIN="$1"
ISOCK="rfc_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
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
echo "flags at rest:      [$(inner list-clients -F '#{client_flags}')]"
inner refresh-client -c "$c" -f no-output >/dev/null; echo "-f no-output rc=$?"
echo "after -f:           [$(inner list-clients -F '#{client_flags}')]"
inner refresh-client -c "$c" -f '' >/dev/null
echo "after clearing:     [$(inner list-clients -F '#{client_flags}')]"
echo "size before -C:     $(inner list-clients -F '#{client_width}x#{client_height}')"
inner refresh-client -c "$c" -C 30x6 >/dev/null; echo "-C rc=$?"
for _ in $(seq 1 25); do
  [ "$(inner list-clients -F '#{client_width}')" = 30 ] && break
  sleep 0.2
done
echo "size after -C:      $(inner list-clients -F '#{client_width}x#{client_height}')"
inner refresh-client -c "$c" -S >/dev/null; echo "-S rc=$?"
inner refresh-client -c "$c" >/dev/null; echo "bare rc=$?"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
