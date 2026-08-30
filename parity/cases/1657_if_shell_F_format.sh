# -F treats the argument as a format instead of running a shell: it is true when
# it expands to something other than empty or "0".
$TM set -g @r none
$TM if-shell -F '1' 'set -g @r true-branch' 'set -g @r false-branch'
echo "1 -> $($TM show -gv @r)"
$TM if-shell -F '0' 'set -g @r true-branch' 'set -g @r false-branch'
echo "0 -> $($TM show -gv @r)"
$TM if-shell -F '' 'set -g @r true-branch' 'set -g @r false-branch'
echo "empty -> $($TM show -gv @r)"
$TM if-shell -F '#{==:#{session_windows},1}' 'set -g @r one-window' 'set -g @r more-windows'
echo "format -> $($TM show -gv @r)"
