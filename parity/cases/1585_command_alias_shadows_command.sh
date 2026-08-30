# The alias table is consulted before the command table, so an alias may take
# over the name of a real command.
$TM list-windows -F '#{window_name}'
$TM set -s command-alias[80] 'list-windows=display-message -p hijacked'
$TM list-windows -F '#{window_name}' 2>&1; echo "rc=$?"
$TM set -su command-alias[80]
$TM list-windows -F '#{window_name}'
