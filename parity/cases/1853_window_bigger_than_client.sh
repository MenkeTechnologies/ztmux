# When the window is larger than the client showing it, #{window_bigger} turns
# on and #{window_offset_x}/#{window_offset_y} say which part the client is
# looking at. window-size manual is what allows the mismatch.
set -- $TM
BIN="$1"
ISOCK="wbg_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 10 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner setw -g window-size manual >/dev/null
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
echo "client: $(inner list-clients -F '#{client_width}x#{client_height}')"
echo "window fits:   bigger=[$(inner display-message -p '#{window_bigger}')] offset=[$(inner display-message -p '#{window_offset_x},#{window_offset_y}')]"
inner resize-window -x 120 -y 40 >/dev/null
for _ in $(seq 1 25); do
  [ "$(inner display-message -p '#{window_width}')" = 120 ] && break
  sleep 0.2
done
echo "window 120x40: bigger=[$(inner display-message -p '#{window_bigger}')] offset=[$(inner display-message -p '#{window_offset_x},#{window_offset_y}')]"
echo "== refresh-client moves the visible part =="
c=$(inner list-clients -F '#{client_name}' | head -1)
inner refresh-client -c "$c" -R >/dev/null 2>&1; echo "-R rc=$?"
inner refresh-client -c "$c" -D >/dev/null 2>&1; echo "-D rc=$?"
echo "offset now: [$(inner display-message -p '#{window_offset_x},#{window_offset_y}')]"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
