# window-size decides the size of a window with more than one client attached:
# largest, smallest, latest or manual. Two clients of different sizes are
# attached to the same inner session, and the window's own size is read back.
set -- $TM
BIN="$1"
ISOCK="wsz_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s inner -x 40 -y 10 'sleep 300' >/dev/null
inner set -g status off >/dev/null

$TM new-window -d -n small "$BIN -L $ISOCK attach -t inner"
$TM resize-window -t small -x 40 -y 10 2>/dev/null || true
$TM new-window -d -n big "$BIN -L $ISOCK attach -t inner"
$TM resize-window -t big -x 70 -y 20 2>/dev/null || true
for _ in $(seq 1 25); do
  [ "$(inner list-clients -F x | wc -l | tr -d ' ')" = 2 ] && break
  sleep 0.2
done
echo "clients: $(inner list-clients -F '#{client_width}x#{client_height}' | sort | tr '\n' ' ')"
for v in largest smallest latest; do
  inner setw -g window-size "$v" >/dev/null
  sleep 0.6
  printf '%-9s window=%s\n' "$v" "$(inner display-message -p '#{window_width}x#{window_height}')"
done
inner setw -g window-size manual >/dev/null
inner resize-window -x 55 -y 15 >/dev/null
sleep 0.4
printf '%-9s window=%s\n' manual "$(inner display-message -p '#{window_width}x#{window_height}')"
inner kill-server >/dev/null 2>&1
$TM kill-window -t small 2>/dev/null; $TM kill-window -t big 2>/dev/null
