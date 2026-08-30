# ; separates commands in one invocation; a ;-terminated list is allowed, and a
# literal semicolon reaches the command when escaped.
$TM set -g @a one \; set -g @b two \;
echo "a=$($TM show -gv @a) b=$($TM show -gv @b)"
$TM set -g @semi 'x;y'
echo "semi=$($TM show -gv @semi)"
$TM set -g @esc "x\;y"
echo "esc=$($TM show -gv @esc)"
