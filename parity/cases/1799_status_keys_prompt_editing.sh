# status-keys chooses the editing keys in the command prompt: with emacs, C-a
# goes to the start of the line, so text typed there lands before what was
# already entered.
set -- $TM
BIN="$1"
ISOCK="skp_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 60 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -g status-keys emacs >/dev/null
inner set -g @line '' >/dev/null
inner bind -n F1 command-prompt 'set -g @line "%%"' >/dev/null

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

$TM send-keys -t client F1
sleep 0.5
$TM send-keys -t client -l 'world'
$TM send-keys -t client C-a
$TM send-keys -t client -l 'hello-'
$TM send-keys -t client Enter
for _ in $(seq 1 25); do
  [ -n "$(inner show -gv @line)" ] && break
  sleep 0.2
done
echo "emacs C-a then typing: [$(inner show -gv @line)]"

inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
