# set -u removes an option so the value falls back to the default; unsetting a
# global that was never set is quiet, and -u with a value is a usage error.
echo "default: $($TM show-options -gv status-interval)"
$TM set -g status-interval 7
echo "after set: $($TM show-options -gv status-interval)"
$TM set -gu status-interval; echo "unset rc=$?"
echo "after unset: $($TM show-options -gv status-interval)"
$TM set -gu status-interval; echo "unset again rc=$?"
