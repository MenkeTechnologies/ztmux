# -o refuses to overwrite an option that is already set, and on an array it
# looks at the specific index rather than the whole option.
$TM set -s 'command-alias[20]' 'ztone=display-message -p one'
$TM set -so 'command-alias[20]' 'zttwo=display-message -p two' 2>&1; echo "same index rc=$?"
$TM show -sv 'command-alias[20]'
$TM set -so 'command-alias[21]' 'ztthree=display-message -p three'; echo "free index rc=$?"
$TM show -sv 'command-alias[21]'
$TM set -soq 'command-alias[20]' 'ztfour=display-message -p four'; echo "with -q rc=$?"
$TM show -sv 'command-alias[20]'
$TM set -su 'command-alias[20]'; $TM set -su 'command-alias[21]'
