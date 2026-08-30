# With remain-on-exit on, a pane whose process exits stays as a dead pane and
# records how it died: #{pane_dead_status} for a normal exit, #{pane_dead_signal}
# for a signal. Poll for the pane to die rather than sleeping a fixed time.
$TM setw remain-on-exit on
$TM split-window -d "sh -c 'exit 3'"
$TM split-window -d "sh -c 'kill -TERM \$\$'"
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
  n=$($TM list-panes -F '#{pane_dead}' | grep -c '^1$')
  [ "$n" = 2 ] && break
  sleep 0.2
done
$TM list-panes -F 'dead=#{pane_dead} status=[#{pane_dead_status}] signal=[#{pane_dead_signal}]' | sort
