# An array option prints one line per populated index, -v gives a single index's
# value, and asking for the whole array with -v joins what is there.
$TM set -s 'command-alias[10]' 'ztfirst=display-message -p first'
$TM set -s 'command-alias[11]' 'ztsecond=display-message -p second'
echo "listing:"; $TM show -s command-alias | grep ztfirst -A 0
echo "one index -v: [$($TM show -sv 'command-alias[10]')]"
echo "missing index -v: [$($TM show -sv 'command-alias[999]' 2>&1)]"; echo "rc=$?"
echo "whole array -v: [$($TM show -sv command-alias | head -c 40)]"
$TM set -su 'command-alias[10]'
$TM set -su 'command-alias[11]'
