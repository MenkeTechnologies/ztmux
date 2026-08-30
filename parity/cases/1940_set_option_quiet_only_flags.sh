# -q turns the three failure paths of set-option into silence with a normal exit
# (cmd-set-option.c:113, 134, 160), -o refuses to overwrite an option that is
# already set, and -U removes the option from every pane in the window.
$TM set -q -g no-such-option value; echo "invalid option with -q: rc=$?"
$TM set -g no-such-option value 2>&1; echo "without -q: rc=$?"
$TM set -q -g '@once' first; echo "first set: rc=$?"
$TM set -o -g '@once' second 2>&1; echo "-o over an existing option: rc=$?"
echo "value kept: [$($TM show -gv '@once')]"
$TM set -o -q -g '@once' third; echo "-o -q: rc=$?"
echo "value still: [$($TM show -gv '@once')]"
echo "== -o on an option that is not set yet succeeds =="
$TM set -o -g '@fresh' ok; echo "rc=$?"
echo "[$($TM show -gv '@fresh')]"
echo "== ambiguous names are reported apart from invalid ones =="
$TM set -g stat x 2>&1; echo "rc=$?"
$TM set -q -g stat x; echo "ambiguous with -q: rc=$?"
