# An unknown command name is rejected by name, and the failure is per-command:
# the rest of a ;-separated list still runs.
$TM frobnicate 2>&1; echo "rc=$?"
$TM set -g @x before \; frobnicate \; set -g @x after 2>&1; echo "rc=$?"
echo "@x=$($TM show -gv @x)"
