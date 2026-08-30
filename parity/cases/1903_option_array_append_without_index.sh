# -a on an array option without a subscript appends a new element rather than
# concatenating onto the last one.
$TM set -s 'terminal-features[0]' 'aaa*:RGB'
$TM show -s terminal-features | sort
$TM set -sa terminal-features 'bbb*:256'
echo "after -a with no index:"
$TM show -s terminal-features | sort
$TM set -sa 'terminal-features[0]' ':overline'
echo "after -a with an index:"
$TM show -s terminal-features | sort
$TM set -su terminal-features
