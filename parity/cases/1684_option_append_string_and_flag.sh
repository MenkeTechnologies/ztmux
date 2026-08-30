# -a on a plain string option concatenates; on a flag or a number it is an error.
$TM set -g @s 'one'
$TM set -ga @s '-two'
echo "string: $($TM show -gv @s)"
$TM set -g status-left 'L'
$TM set -ga status-left 'R'
echo "status-left: $($TM show -gv status-left)"
echo "== appending to a number =="
$TM set -ga history-limit 5 2>&1; echo "rc=$?"
echo "== appending to a flag =="
$TM set -ga status on 2>&1; echo "rc=$?"
