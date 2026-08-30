# pane-colours is an array of colours and user-keys an array of key strings;
# both take subscripts, append, and reject a bad element.
echo "pane-colours default: [$($TM show -gwv pane-colours 2>&1)]"
$TM setw -g 'pane-colours[0]' red
$TM setw -g 'pane-colours[1]' '#0000ff'
$TM show -gw pane-colours | sort
$TM setw -g 'pane-colours[0]' notacolour 2>&1; echo "rc=$?"
$TM setw -gu pane-colours
echo "user-keys default: [$($TM show -sv user-keys 2>&1)]"
$TM set -s 'user-keys[0]' '\033[1;2A'
$TM show -sv 'user-keys[0]'
$TM set -sa 'user-keys[0]' 'X'
$TM show -sv 'user-keys[0]'
$TM set -su user-keys
