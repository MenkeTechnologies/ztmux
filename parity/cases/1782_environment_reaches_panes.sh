# A variable set in the session environment is in the environment of a pane
# spawned afterwards; one removed with -r is not, and -u takes it out again.
# The pane prints its own environment, which is read back with capture-pane.
$TM set -g status off
$TM set-environment ZTPAR_KEPT kept-value
$TM set-environment -r ZTPAR_REMOVED
$TM set-environment ZTPAR_GONE will-be-unset
$TM set-environment -u ZTPAR_GONE
$TM split-window -d 'printf "kept=[%s] gone=[%s] removed=[%s]\n" "$ZTPAR_KEPT" "$ZTPAR_GONE" "$ZTPAR_REMOVED"; sleep 300'
pane=$($TM list-panes -F '#{pane_id}' | tail -1)
for _ in $(seq 1 40); do
  [ "$($TM capture-pane -p -t "$pane" | grep -c kept=)" = 1 ] && break
  sleep 0.2
done
$TM capture-pane -p -t "$pane" | head -1
echo "== and the server's own view =="
$TM show-environment ZTPAR_KEPT
$TM show-environment ZTPAR_GONE 2>&1; echo "rc=$?"
