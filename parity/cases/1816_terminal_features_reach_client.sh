# terminal-features adds capabilities for a TERM pattern, and a client whose
# TERM matches picks them up -- which #{I/f:...} reports. This is the option and
# the modifier meeting: the option is set BEFORE the client attaches, since the
# features are resolved at attach time.
set -- $TM
BIN="$1"
ISOCK="tfc_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 6 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -s 'terminal-features[0]' '*:overline' >/dev/null
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
echo "overline added by the option: [$(inner display-message -p -c "$c" '#{I/f:overline}')]"
echo "a feature not added:          [$(inner display-message -p -c "$c" '#{I/f:sixel}')]"
echo "a capability from the TERM:   [$(inner display-message -p -c "$c" '#{I/c:cup}')]"
echo "a capability it lacks:        [$(inner display-message -p -c "$c" '#{I/c:notacap}')]"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
