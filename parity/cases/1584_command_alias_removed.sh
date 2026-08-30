# Removing an alias index removes the command name with it.
$TM set -s command-alias[60] 'zzgone=display-message -p bye'
$TM zzgone; echo "rc=$?"
$TM set -su command-alias[60]; echo "unset rc=$?"
$TM zzgone 2>&1; echo "rc=$?"
