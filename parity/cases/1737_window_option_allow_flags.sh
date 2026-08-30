# allow-rename, allow-set-title and allow-passthrough decide what a program in
# the pane may do to the window. They round-trip as flags, and allow-rename
# actually gates the rename escape sequence.
$TM set -g automatic-rename off
for o in allow-rename allow-set-title allow-passthrough; do
  printf '%-18s default=%s\n' "$o" "$($TM show -gwv "$o")"
done
$TM setw -g allow-rename on
$TM rename-window fixed
$TM split-window -d 'printf "\033kfrom-escape\033\\"; sleep 300'
for _ in $(seq 1 40); do
  [ "$($TM display-message -p '#{window_name}')" = from-escape ] && break
  sleep 0.2
done
echo "with allow-rename on: [$($TM display-message -p '#{window_name}')]"
$TM setw -g allow-rename off
$TM rename-window fixed
$TM split-window -d 'printf "\033kshould-not-take\033\\"; sleep 300'
sleep 1
echo "with allow-rename off: [$($TM display-message -p '#{window_name}')]"
