# terminal-features is a list of capability names; each one the C knows must be
# accepted, and an unknown one must be refused. The names that no other case
# uses are the ones checked first.
$TM set -s 'terminal-features[0]' 'ztpar*:bpaste:ccolour:cstyle:ignorefkeys:margins'
$TM show -sv 'terminal-features[0]'
$TM set -s 'terminal-features[1]' 'ztpar*:osc7:overline:rectfill:usstyle:sync'
$TM show -sv 'terminal-features[1]'
$TM set -s 'terminal-features[2]' 'ztpar*:256:RGB:clipboard:focus:hyperlinks:mouse:sixel:strikethrough:title:extkeys:progressbar'
$TM show -sv 'terminal-features[2]'
echo "== an unknown capability =="
$TM set -s 'terminal-features[3]' 'ztpar*:notafeature' 2>&1; echo "rc=$?"
$TM show -s terminal-features | wc -l | tr -d ' '
$TM set -su terminal-features
