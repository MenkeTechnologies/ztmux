# visual-activity on makes the alert show as a message to the client rather than
# only setting the window flag; the message lands in the client's message log.
set -- $TM
BIN="$1"
ISOCK="vis_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -n first -x 40 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -g automatic-rename off >/dev/null
inner setw -g monitor-activity on >/dev/null
inner set -g visual-activity on >/dev/null
inner new-window -d -n noisy 'sleep 300' >/dev/null
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
inner send-keys -t noisy -l 'echo something' >/dev/null
inner send-keys -t noisy Enter >/dev/null
for _ in $(seq 1 25); do
  [ "$(inner show-messages -F '#{message_text}' 2>/dev/null | grep -ci activity)" -ge 1 ] && break
  sleep 0.2
done
echo "message log mentions activity: $(inner show-messages -F '#{message_text}' | grep -ci activity)"
echo "window flag: $(inner display-message -p -t noisy '#{window_activity_flag}')"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
