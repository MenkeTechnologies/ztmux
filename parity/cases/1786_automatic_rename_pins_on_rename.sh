# Renaming a window by hand turns automatic-rename off for that window, so the
# name stays put afterwards. The name automatic-rename would have chosen is NOT
# compared: it comes from whatever process the pty reports, which differs by
# machine and by shell.
$TM set -g status off
$TM setw -g automatic-rename on
$TM new-window -d 'sleep 300'
win=$($TM list-windows -F '#{window_index}' | tail -1)
echo "option before renaming: [$($TM show -wv -t "$win" automatic-rename)] (unset means the global on)"
$TM rename-window -t "$win" pinned
echo "name after renaming:   [$($TM display-message -p -t "$win" '#{window_name}')]"
echo "option after renaming: [$($TM show -wv -t "$win" automatic-rename)]"
sleep 1
echo "name a moment later:   [$($TM display-message -p -t "$win" '#{window_name}')]"
$TM setw -gu automatic-rename
