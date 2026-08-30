# switch-client -T sets the client's key table, which #{client_key_table} reads
# back; a key bound in that table then fires without any prefix, and the table
# resets to root after a key that does not repeat.
set -- $TM
BIN="$1"
ISOCK="sct_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 6 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -g @hit none >/dev/null
inner bind -T mytable a 'set -g @hit table-a' >/dev/null

$TM new-window -d -n client "$BIN -L $ISOCK attach -t inner"
for _ in $(seq 1 60); do
  [ "$(inner display-message -p '#{session_attached}')" = 1 ] && break
  sleep 0.2
done
# If the client did not attach in time -- a slow or loaded machine, not a
# divergence -- say so once and stop, so both binaries produce the same short
# output instead of diverging further down error paths.
[ "$(inner display-message -p '#{session_attached}')" = 1 ] || {
  echo "client did not attach"; inner kill-server >/dev/null 2>&1; exit 0; }
c=$(inner list-clients -F '#{client_name}' | head -1)
echo "key table at rest: [$(inner list-clients -F '#{client_key_table}')]"
inner switch-client -c "$c" -T mytable >/dev/null
echo "after -T mytable:  [$(inner list-clients -F '#{client_key_table}')]"
$TM send-keys -t client a
for _ in $(seq 1 60); do
  [ "$(inner show -gv @hit)" = table-a ] && break
  sleep 0.2
done
echo "the bound key ran: [$(inner show -gv @hit)]"
echo "table afterwards:  [$(inner list-clients -F '#{client_key_table}')]"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
