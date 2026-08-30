# An unknown flag prints the command's usage line; a flag that needs an argument
# and has none does too.
$TM list-windows -Z 2>&1; echo "rc=$?"
$TM list-windows -F 2>&1; echo "rc=$?"
$TM display-message -p -t 2>&1; echo "rc=$?"
