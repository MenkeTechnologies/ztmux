# An environment value may contain '=' and may be empty; show-environment prints
# a variable with no value differently from one that is unset.
$TM set-environment ZTPAR_EQ 'a=b=c'
$TM show-environment ZTPAR_EQ
$TM set-environment ZTPAR_EMPTY ''
$TM show-environment ZTPAR_EMPTY
$TM show-environment ZTPAR_NOT_SET 2>&1; echo "rc=$?"
echo "== -r marks a variable for removal, which prints with a leading dash =="
$TM set-environment -r ZTPAR_REMOVED
$TM show-environment | grep '^-ZTPAR_REMOVED'
echo "== unsetting =="
$TM set-environment -u ZTPAR_EQ
$TM show-environment ZTPAR_EQ 2>&1; echo "rc=$?"
