# -v prints just the value; -q silences the error for an option that is not set.
$TM set -g @val hello
echo "value: $($TM show-options -gv @val)"
echo "== unknown user option, plain =="
$TM show-options -gv @nosuch 2>&1; echo "rc=$?"
echo "== unknown user option, -q =="
$TM show-options -gvq @nosuch 2>&1; echo "rc=$?"
echo "== unknown option name =="
$TM show-options -g not-an-option 2>&1; echo "rc=$?"
