# -F expands the value as a format when the option is set (cmd-set-option.c:39),
# so the stored value is the expansion, not the format text.
$TM set -g @plain '#{session_windows}'
$TM set -gF @expanded '#{session_windows}'
echo "plain=$($TM show -gv @plain)"
echo "expanded=$($TM show -gv @expanded)"
$TM set -gF @arith '#{e|*|:6,7}'
echo "arith=$($TM show -gv @arith)"
