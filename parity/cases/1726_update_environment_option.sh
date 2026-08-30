# update-environment is an array option listing the variables copied from a
# client's environment when it attaches; it appends and unsets like any array.
$TM show -gv update-environment | tr ' ' '\n' | sort | head -3
$TM set -ga update-environment 'ZTPAR_EXTRA'
$TM show -gv update-environment | tr ' ' '\n' | grep ZTPAR_EXTRA
$TM set -g 'update-environment[0]' 'ONLY_THIS'
$TM show -gv 'update-environment[0]'
$TM set -gu update-environment
$TM show -gv update-environment | tr ' ' '\n' | grep -c ZTPAR_EXTRA
