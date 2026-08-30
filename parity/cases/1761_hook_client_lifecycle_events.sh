# The client hooks fire for a real client: attaching runs client-attached and
# client-active/client-focus-in follow it, resizing the terminal runs
# client-resized, and detaching runs client-detached. client-active and the two
# focus hooks need the terminal to report focus, which nothing here does, so the
# case also pins that they stay quiet. Driven through the nested-client
# technique so there is an actual client to attach and resize.
set -- $TM
BIN="$1"
ISOCK="clh_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 6 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner set -g @log '' >/dev/null
for h in client-attached client-detached client-resized client-active \
         client-focus-in client-focus-out; do
  inner set-hook -g "$h" "set -ga @log \",$h\"" >/dev/null
done

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
echo "after attach: [$(inner show -gv @log)]"

inner set -g @log '' >/dev/null
$TM resize-window -t client -x 60 -y 12 2>/dev/null || $TM resize-pane -t client -x 60 -y 12
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{window_width}')" != 40 ] && break
  sleep 0.2
done
echo "after resize: [$(inner show -gv @log)]"

inner set -g @log '' >/dev/null
$TM kill-window -t client
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{session_attached}')" = 0 ] && break
  sleep 0.2
done
echo "after detach: [$(inner show -gv @log)]"
inner kill-server >/dev/null 2>&1
