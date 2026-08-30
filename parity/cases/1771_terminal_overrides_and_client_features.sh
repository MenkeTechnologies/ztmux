# terminal-overrides carries raw capability strings for a TERM pattern; both it
# and terminal-features round-trip as arrays and are matched against the
# client's TERM, so with no client they only have to store and give back.
$TM show -sv terminal-overrides | head -2
$TM set -s 'terminal-overrides[0]' 'ztpar*:Cs=\E]12;%p1%s\007'
$TM show -sv 'terminal-overrides[0]'
$TM set -sa 'terminal-overrides[0]' ':Cr=\E]112\007'
$TM show -sv 'terminal-overrides[0]'
$TM set -su terminal-overrides
echo "== the default-terminal option =="
$TM show -sv default-terminal
$TM set -s default-terminal 'screen-256color'; $TM show -sv default-terminal
$TM set -su default-terminal
