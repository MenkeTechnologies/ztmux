# command-prompt with a client behind it: the prompt takes input, %1 and %% are
# replaced with what was typed, and -I pre-fills the line. Driven through the
# nested-client technique, with the typed text going to the OUTER pane -- which
# is the inner client's terminal input.
set -- $TM
BIN="$1"
ISOCK="cpd_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 60 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -g @answer '' >/dev/null
inner bind -n F1 command-prompt -p 'word:' 'set -g @answer "got %1"' >/dev/null
inner bind -n F2 command-prompt -I 'prefilled' 'set -g @answer "line %%"' >/dev/null

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
$TM send-keys -t client -l 'hello'
$TM send-keys -t client Enter
for _ in $(seq 1 25); do
  [ -n "$(inner show -gv @answer)" ] && break
  sleep 0.2
done
echo "with -p: [$(inner show -gv @answer)]"

inner set -g @answer '' >/dev/null
$TM send-keys -t client F2
sleep 0.5
$TM send-keys -t client Enter
for _ in $(seq 1 25); do
  [ -n "$(inner show -gv @answer)" ] && break
  sleep 0.2
done
echo "with -I: [$(inner show -gv @answer)]"

inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
