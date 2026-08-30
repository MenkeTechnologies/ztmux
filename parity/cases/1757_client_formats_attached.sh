# The client_* formats need a client, so this uses the suite's nested-client
# technique (as cases 1504/1507/1508 do): an inner server with a client attached
# from a pane of the outer one, then the inner server is asked about that client.
#
# Split by what can be compared: the terminal- and user-derived values are the
# same for both binaries (same TERM, same user, same host, same pane geometry),
# while the pid, the tty name, the creation time and the byte counters differ by
# construction and are reduced to their shape.
set -- $TM
BIN="$1"
ISOCK="cfm_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }
shape() { perl -pe 's{^/dev/tty\S+$}{TTY-SHAPE-OK}; s/^\d+$/NUMBER-SHAPE-OK/; s/^$/EMPTY/'; }

inner -f /dev/null new-session -d -s inner -x 40 -y 6 'sleep 300' >/dev/null
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

echo "== comparable directly =="
for f in client_control_mode client_key_table client_prefix client_utf8 \
         client_termname client_user client_uid \
         client_cell_width client_cell_height client_colours client_theme \
         client_last_session client_flags; do
  printf '%-22s %s\n' "$f" "$(inner list-clients -F "#{$f}")"
done

# client_termtype carries the version string ("tmux next-3.7" against
# "tmux 3.7.45"), which the suite's determinism rules keep out of comparisons the
# same way #{version} is kept out; only its shape is checked.
echo "== by shape only =="
for f in client_tty client_pid client_created client_written client_discarded; do
  printf '%-22s %s\n' "$f" "$(inner list-clients -F "#{$f}" | shape)"
done
printf '%-22s %s\n' client_termtype "$(inner list-clients -F '#{client_termtype}' | perl -pe 's/^tmux .+$/TERMTYPE-SHAPE-OK/')"

inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
