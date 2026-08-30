# A mouse drag inside copy mode selects text: press, move with the button held,
# release. The events are real SGR sequences written into the inner client's
# terminal, and the selection is read back through #{selection_present} and the
# buffer the release leaves behind.
set -- $TM
BIN="$1"
ISOCK="mdr_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }
hex() { printf '%s' "$1" | perl -ne 'print join(" ", map { sprintf "%02x", ord } split //)'; }
# button 0 press / drag (32) / release at column $1 row $2
press()   { $TM send-keys -t client -H 1b 5b 3c 30 3b $(hex "$1") 3b $(hex "$2") 4d; }
drag()    { $TM send-keys -t client -H 1b 5b 3c 33 32 3b $(hex "$1") 3b $(hex "$2") 4d; }
release() { $TM send-keys -t client -H 1b 5b 3c 30 3b $(hex "$1") 3b $(hex "$2") 6d; }

inner -f /dev/null new-session -d -s inner -x 40 -y 8 "printf 'abcdefghij\n'; sleep 300" >/dev/null
inner set -g status off >/dev/null
inner set -g mouse on >/dev/null
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
for _ in $(seq 1 25); do
  [ -n "$(inner display-message -p '#{pane_mode}')" ] && break
  sleep 0.2
done
echo "mode: [$(inner display-message -p '#{pane_mode}')] selection before: $(inner display-message -p '#{selection_present}')"
press 1 1
drag 5 1
echo "selection while dragging: $(inner display-message -p '#{selection_present}')"
release 5 1
for _ in $(seq 1 25); do
  [ -n "$(inner show-buffer 2>/dev/null)" ] && break
  sleep 0.2
done
echo "buffer after the release: [$(inner show-buffer 2>&1)]"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
