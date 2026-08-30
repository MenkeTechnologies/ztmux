# remain-on-exit gained a fourth choice in next-3.7: "key" (3) keeps the dead
# pane until a key is pressed (options-table.c:93-95, server-fn.c:339-350), and
# the key that dismisses it sets the option back to off (server-client.c:1557-1566).
$TM set -g status off
echo "choices, by setting each one:"
for v in off on failed key; do
  $TM set -w remain-on-exit "$v" 2>&1 && echo "  $v -> [$($TM show -wv remain-on-exit)]"
done
$TM set -w remain-on-exit nosuchchoice 2>&1; echo "rc=$?"
echo "== a pane that exits under 'key' stays dead rather than closing =="
$TM set -w remain-on-exit key
$TM split-window -d 'exit 7'
pane=%$($TM list-panes -F '#{pane_id}' | tr -d '%' | sort -n | tail -1)
for _ in $(seq 1 25); do
  [ "$($TM display-message -p -t "$pane" '#{pane_dead}')" = 1 ] && break
  sleep 0.2
done
echo "dead: $($TM display-message -p -t "$pane" '#{pane_dead}') status: $($TM display-message -p -t "$pane" '#{pane_dead_status}')"
echo "panes: $($TM list-panes | wc -l | tr -d ' ')"
echo "the option is still key: [$($TM show -w -t "$pane" -v remain-on-exit)]"
echo "== 'off' closes the pane instead =="
$TM set -w remain-on-exit off
$TM split-window -d 'exit 0'
for _ in $(seq 1 25); do
  [ "$($TM list-panes | wc -l | tr -d ' ')" = 2 ] && break
  sleep 0.2
done
echo "panes: $($TM list-panes | wc -l | tr -d ' ')"
