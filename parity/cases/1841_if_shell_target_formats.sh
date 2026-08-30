# -t gives if-shell the pane its formats expand against, so the same -F test can
# come out differently for two panes.
$TM set -g automatic-rename off
$TM new-window -d -n one 'sleep 300'
$TM new-window -d -n two 'sleep 300'
$TM split-window -d -t two
$TM set -g @r none
$TM if-shell -F -t one '#{==:#{window_panes},1}' 'set -g @r one-has-one-pane' 'set -g @r one-has-more'
echo "against window one: $($TM show -gv @r)"
$TM if-shell -F -t two '#{==:#{window_panes},1}' 'set -g @r two-has-one-pane' 'set -g @r two-has-more'
echo "against window two: $($TM show -gv @r)"
echo "== and the shell form takes the target too =="
$TM if-shell -t two 'true' 'set -g @r shell-true' 'set -g @r shell-false'
echo "shell form: $($TM show -gv @r)"
