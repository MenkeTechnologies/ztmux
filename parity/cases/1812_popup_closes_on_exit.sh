# display-popup -E closes the popup when its command exits; without -E the popup
# stays until a key closes it. #{client_flags} does not say, but the pane the
# popup covers is redrawn, so the popup's own output is what tells them apart.
set -- $TM
BIN="$1"
ISOCK="pop_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }
seen() { $TM capture-pane -p -t client | grep -c 'POPTEXT'; }

inner -f /dev/null new-session -d -s inner -x 40 -y 10 'sleep 300' >/dev/null
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

inner display-popup -E -w 20 -h 5 "printf 'POPTEXT\n'" >/dev/null 2>&1 &
for _ in $(seq 1 25); do
  [ "$(seen)" -ge 1 ] && break
  sleep 0.2
done
echo "with -E, popup content on screen: $(seen)"
for _ in $(seq 1 25); do
  [ "$(seen)" = 0 ] && break
  sleep 0.2
done
echo "after the command exits:          $(seen)"

inner display-popup -w 20 -h 5 "printf 'POPTEXT\n'" >/dev/null 2>&1 &
for _ in $(seq 1 25); do
  [ "$(seen)" -ge 1 ] && break
  sleep 0.2
done
echo "without -E, still on screen:      $(seen)"
$TM send-keys -t client q
for _ in $(seq 1 25); do
  [ "$(seen)" = 0 ] && break
  sleep 0.2
done
echo "after a key:                      $(seen)"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
