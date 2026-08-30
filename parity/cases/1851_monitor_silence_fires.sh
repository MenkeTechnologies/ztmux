# monitor-silence flags a window that has produced no output for the given
# number of seconds, and alert-silence fires with it. The interval is one
# second so the case can wait for it without stretching the runner's budget.
$TM set -g automatic-rename off
$TM set -g status off
$TM set -g @log ''
$TM set-hook -g alert-silence 'set -ga @log ",silence"'
$TM setw -g monitor-silence 1
$TM new-window -d -n quiet 'sleep 300'
echo "flag right away: $($TM display-message -p -t quiet '#{window_silence_flag}')"
for _ in $(seq 1 25); do
  [ "$($TM display-message -p -t quiet '#{window_silence_flag}')" = 1 ] && break
  sleep 0.2
done
echo "flag after the interval: $($TM display-message -p -t quiet '#{window_silence_flag}')"
echo "hook: [$($TM show -gv @log)]"
$TM setw -g monitor-silence 0
$TM set-hook -gu alert-silence
