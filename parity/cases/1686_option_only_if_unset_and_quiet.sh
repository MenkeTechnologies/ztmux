# -o refuses to overwrite an option that is already set; -q silences the refusal.
$TM set -g @o first
$TM set -go @o second 2>&1; echo "rc=$?"
echo "value=$($TM show -gv @o)"
$TM set -goq @o third 2>&1; echo "quiet rc=$?"
echo "value=$($TM show -gv @o)"
$TM set -gu @o
$TM set -go @o fresh; echo "unset-then--o rc=$?"
echo "value=$($TM show -gv @o)"
