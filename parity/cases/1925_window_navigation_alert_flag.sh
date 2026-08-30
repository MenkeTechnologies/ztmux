# next-window -a and previous-window -a move only to a window with an alert, and
# say so when there is none.
$TM set -g automatic-rename off
$TM set -g status off
$TM setw -g monitor-activity on
$TM new-window -d -n quiet1
$TM new-window -d -n noisy 'printf "output\n"; sleep 300'
$TM new-window -d -n quiet2
$TM select-window -t 0
for _ in $(seq 1 25); do
  [ "$($TM display-message -p -t noisy '#{window_activity_flag}')" = 1 ] && break
  sleep 0.2
done
echo "alerted window: $($TM list-windows -F '#{window_name}:#{window_activity_flag}' | grep ':1$' | tr '\n' ' ')"
$TM next-window -a; echo "next -a rc=$?"
echo "current: $($TM display-message -p '#{window_name}')"
$TM select-window -t 0
$TM previous-window -a; echo "previous -a rc=$?"
echo "current: $($TM display-message -p '#{window_name}')"
$TM setw -g monitor-activity off
