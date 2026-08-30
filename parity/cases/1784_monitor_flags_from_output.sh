# monitor-activity sets the window's activity flag when an inactive window
# produces output, and monitor-bell sets the bell flag when it emits a BEL --
# from the pane's OUTPUT, which is what a program in it writes, not from keys
# sent to it. The alert hooks fire alongside.
$TM set -g automatic-rename off
$TM set -g status off
$TM set -g @log ''
$TM set-hook -g alert-activity 'set -ga @log ",activity"'
$TM set-hook -g alert-bell 'set -ga @log ",bell"'
$TM setw -g monitor-activity on
$TM setw -g monitor-bell on
$TM new-window -d -n noisy 'printf "output\n"; sleep 300'
for _ in $(seq 1 25); do
  [ "$($TM display-message -p -t noisy '#{window_activity_flag}')" = 1 ] && break
  sleep 0.2
done
$TM display-message -p -t noisy 'after output: activity=#{window_activity_flag}'
$TM new-window -d -n belly 'printf "\a"; sleep 300'
for _ in $(seq 1 25); do
  [ "$($TM display-message -p -t belly '#{window_bell_flag}')" = 1 ] && break
  sleep 0.2
done
$TM display-message -p -t belly 'after bell: bell=#{window_bell_flag}'
echo "hooks: [$($TM show -gv @log | perl -pe 's/(,activity)+/,activity/; s/(,bell)+/,bell/')]"
$TM set-hook -gu alert-activity; $TM set-hook -gu alert-bell
