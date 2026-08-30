# activity-action and bell-action decide WHICH sessions are alerted: `any` for
# every session showing the window, `current` only for the session whose current
# window it is, `other` only for the others, and `none` for no one.
$TM set -g automatic-rename off
$TM set -g status off
$TM set -g @log ''
$TM set-hook -g alert-activity 'set -ga @log ",#{hook_session_name}"'
$TM setw -g monitor-activity on
$TM new-session -d -s other -x 80 -y 24
$TM new-window -d -n noisy 'sleep 300'
$TM link-window -s noisy -t other:9
probe() {
  $TM set -g @log ''
  $TM setw -t noisy monitor-activity on
  $TM send-keys -t noisy -l "echo $1"
  $TM send-keys -t noisy Enter
  for _ in $(seq 1 25); do
    [ -n "$($TM show -gv @log)" ] && break
    sleep 0.2
  done
  printf '%-8s log=[%s]\n' "$1" "$($TM show -gv @log)"
}
for action in any none; do
  $TM set -g activity-action "$action"
  probe "$action"
done
$TM set -gu activity-action
$TM set-hook -gu alert-activity
