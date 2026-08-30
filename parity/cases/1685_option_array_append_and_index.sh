# Array options: -a appends to the value at an index, a bare index sets just that
# entry, and show prints one line per populated index.
$TM set -s 'terminal-features[0]' 'screen*:title'
$TM set -s 'terminal-features[1]' 'xterm*:RGB'
$TM show -s terminal-features | sort
echo "== -a appends to an entry =="
$TM set -sa 'terminal-features[1]' ':256'
$TM show -sv 'terminal-features[1]'
echo "== -u removes one entry =="
$TM set -su 'terminal-features[0]'
$TM show -s terminal-features | sort
