# remain-on-exit-format is the line a dead pane shows; it is a format, so it
# expands against the pane that died.
echo "default: [$($TM show -gwv remain-on-exit-format)]"
$TM setw -g remain-on-exit on
$TM setw -g remain-on-exit-format 'gone: status=#{pane_dead_status} signal=#{pane_dead_signal}'
$TM split-window -d "sh -c 'exit 7'"
for _ in $(seq 1 40); do
  [ "$($TM list-panes -F '#{pane_dead}' | grep -c '^1$')" = 1 ] && break
  sleep 0.2
done
echo "option now: [$($TM show -gwv remain-on-exit-format)]"
$TM display-message -p -t 1 "expanded: $($TM show -gwv remain-on-exit-format)"
$TM setw -gu remain-on-exit-format
$TM setw -gu remain-on-exit
