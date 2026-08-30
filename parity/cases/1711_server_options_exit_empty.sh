# exit-empty makes the server exit when its last session goes away. Turning it
# off keeps the server alive with no sessions at all, which nothing else in the
# suite exercises. Socket paths are stripped: they differ between the binaries.
# Same race as case 1712: the client either sees the server exit or finds the
# socket already gone. Fold both into one token.
strip() { perl -pe 's{^server exited unexpectedly$}{SERVER GONE}; s{^no server running on /\S+$}{SERVER GONE}'; }
$TM show -sv exit-empty
$TM set -s exit-empty off
$TM show -sv exit-empty
$TM new-session -d -s only -x 80 -y 24
$TM kill-session -t 0
$TM list-sessions -F '#{session_name}'
$TM kill-session -t only
echo "sessions after killing them all: [$($TM list-sessions -F '#{session_name}' 2>&1 | strip)]"
echo "server still answers: [$($TM show -sv exit-empty 2>&1 | strip)]"
