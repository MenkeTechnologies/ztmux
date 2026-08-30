# With mode-keys vi the copy-mode keys are the vi ones: v starts the selection,
# l moves right, y copies and leaves the mode. Driven as real keys through a
# client, which is the only way the key table is consulted at all.
set -- $TM
BIN="$1"
ISOCK="cvi_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 6 "printf 'abcdefgh\n'; sleep 300" >/dev/null
inner set -g status off >/dev/null
inner setw -g mode-keys vi >/dev/null
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
inner copy-mode >/dev/null
inner send-keys -X top-line >/dev/null
inner send-keys -X start-of-line >/dev/null
echo "mode: [$(inner display-message -p '#{pane_mode}')] keys: $(inner show -gwv mode-keys)"
$TM send-keys -t client v
$TM send-keys -t client l
$TM send-keys -t client l
echo "selection present: $(inner display-message -p '#{selection_present}')"
$TM send-keys -t client y
for _ in $(seq 1 25); do
  [ -z "$(inner display-message -p '#{pane_mode}')" ] && break
  sleep 0.2
done
echo "after y: mode=[$(inner display-message -p '#{pane_mode}')] buffer=[$(inner show-buffer 2>&1)]"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
