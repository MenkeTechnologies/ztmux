# send-keys -M forwards a mouse event to the pane, which a program that has
# asked for mouse reporting (DECSET 1000) then receives. The pane runs cat -v so
# whatever arrives is printed as text.
set -- $TM
BIN="$1"
ISOCK="skm_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }
hex() { printf '%s' "$1" | perl -ne 'print join(" ", map { sprintf "%02x", ord } split //)'; }

inner -f /dev/null new-session -d -s inner -x 40 -y 8 'cat -v' >/dev/null
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
# The program asks for mouse reporting and SGR encoding.
inner send-keys -H 1b 5b 3f 31 30 30 30 68 1b 5b 3f 31 30 30 36 68 >/dev/null
sleep 0.4
echo "mouse_any_flag now: $(inner display-message -p '#{mouse_any_flag}')"
# A click in the client's terminal, which tmux forwards because the pane asked.
$TM send-keys -t client -H 1b 5b 3c 30 3b $(hex 3) 3b $(hex 2) 4d
for _ in $(seq 1 25); do
  [ "$(inner capture-pane -p | grep -c '\[<0;')" -ge 1 ] && break
  sleep 0.2
done
echo "the program received a mouse report: $(inner capture-pane -p | grep -c '\[<0;')"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
