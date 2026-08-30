# wait-for blocks until the channel is signalled. The waiter runs in a pane, so
# the case can watch it block and then be released -- the paths the earlier
# wait-for cases could only approach from the non-blocking side.
set -- $TM
BIN="$1"
out="${TMPDIR:-/tmp}/ztpar_waitfor_waiter.out"
command rm -f "$out"
$TM set -g status off
$TM new-window -d -n waiter "$BIN $(printf '%s' "${TM#$BIN }") wait-for ztchan; printf 'released\n' > $out; sleep 300"
sleep 1
echo "while nothing has signalled: [$(cat "$out" 2>/dev/null)]"
$TM wait-for -S ztchan; echo "signal rc=$?"
for _ in $(seq 1 40); do
  [ -s "$out" ] && break
  sleep 0.2
done
echo "after the signal:            [$(cat "$out" 2>/dev/null)]"
command rm -f "$out"
