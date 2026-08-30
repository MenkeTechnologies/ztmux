# -n is shorthand for -T root, and rebinding a key replaces the old command.
$TM bind -n F5 set -g @f5 first
$TM list-keys -T root F5
$TM bind -n F5 set -g @f5 second
$TM list-keys -T root F5
$TM unbind -n F5
$TM list-keys -T root F5 2>&1; echo "rc=$?"
