# switch-client -Z keeps a zoomed pane zoomed across the switch, where a plain
# switch leaves the window unzoomed.
set -- $TM
BIN="$1"
ISOCK="scz_$$_inner"
inner() { $BIN -L "$ISOCK" "$@" 2>&1; }

inner -f /dev/null new-session -d -s one -x 40 -y 8 'sleep 300' >/dev/null
inner new-session -d -s two -x 40 -y 8 'sleep 300' >/dev/null
inner set -g status off >/dev/null
inner split-window -d -t one >/dev/null
inner resize-pane -Z -t one >/dev/null
$TM new-window -d -n client "$BIN -L $ISOCK attach -t one"
for _ in $(seq 1 25); do
  [ "$(inner display-message -p -t one '#{session_attached}')" = 1 ] && break
  sleep 0.2
done
# If the client did not attach in time -- a slow or loaded machine, not a
# divergence -- say so once and stop, so both binaries produce the same short
# output instead of diverging further down error paths.
[ "$(inner display-message -p '#{session_attached}')" = 1 ] || {
  echo "client did not attach"; inner kill-server >/dev/null 2>&1; exit 0; }
c=$(inner list-clients -F '#{client_name}' | head -1)
echo "zoomed before: $(inner display-message -p -t one '#{window_zoomed_flag}')"
inner switch-client -c "$c" -Z -t two >/dev/null; echo "switch -Z rc=$?"
echo "session now:   $(inner list-clients -F '#{client_session}')"
echo "one still zoomed: $(inner display-message -p -t one '#{window_zoomed_flag}')"
inner switch-client -c "$c" -t one >/dev/null
echo "back on one, zoomed: $(inner display-message -p -t one '#{window_zoomed_flag}')"
inner kill-server >/dev/null 2>&1
$TM kill-window -t client 2>/dev/null
